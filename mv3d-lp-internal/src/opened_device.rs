use std::ffi::CString;
use std::fmt;
use std::sync::Arc;

use crate::callback::{CallbackRegistration, ExceptionCallbackSink, FrameCallbackSink};
use crate::driver::Handle;
use crate::error::{Error, InvalidInput, Operation};
use crate::file_transfer::FileProgress;
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::RuntimeCore;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceCleanupError {
    pub stop: Option<Box<Error>>,
    pub close: Option<Box<Error>>,
}

impl fmt::Display for DeviceCleanupError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (&self.stop, &self.close) {
            (Some(stop), Some(close)) => write!(
                formatter,
                "device cleanup failed while stopping ({stop}) and closing ({close})"
            ),
            (Some(stop), None) => write!(formatter, "device cleanup failed while stopping: {stop}"),
            (None, Some(close)) => {
                write!(formatter, "device cleanup failed while closing: {close}")
            }
            (None, None) => formatter.write_str("device cleanup failed without an SDK error"),
        }
    }
}

impl std::error::Error for DeviceCleanupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.stop
            .as_deref()
            .or(self.close.as_deref())
            .map(|error| error as &(dyn std::error::Error + 'static))
    }
}

/// Opened device owning a lease on the initialized native session.
pub struct Device {
    runtime: Arc<RuntimeCore>,
    handle: Option<Handle>,
    measuring: bool,
    image_registration: Option<CallbackRegistration>,
    exception_registration: Option<CallbackRegistration>,
    // The asynchronous FileAccess contract does not say whether native code copies these names.
    file_names: Option<(CString, CString)>,
}

impl Device {
    pub(crate) fn new(runtime: Arc<RuntimeCore>, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            measuring: false,
            image_registration: None,
            exception_registration: None,
            file_names: None,
        }
    }

    /// Starts pull acquisition while keeping the session state on the device.
    pub fn start(&mut self) -> Result<(), Error> {
        self.require_stopped(Operation::StartMeasure)?;
        self.runtime.call(Operation::StartMeasure, |driver| {
            driver.start(self.handle())
        })?;
        self.measuring = true;
        Ok(())
    }

    /// Starts callback acquisition and retains the registration until stop or close.
    pub fn start_callback(&mut self, sink: FrameCallbackSink) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterImageDataCallback;
        self.require_stopped(OPERATION)?;

        let registration = CallbackRegistration::image(sink);
        self.runtime.call(OPERATION, |driver| {
            driver.register_image_callback(self.handle(), registration.cookie())
        })?;
        self.runtime.call(Operation::StartMeasure, |driver| {
            driver.start(self.handle())
        })?;
        self.image_registration = Some(registration);
        self.measuring = true;
        Ok(())
    }

    /// Stops the active acquisition and then retires its image callback registration.
    pub fn stop(&mut self) -> Result<(), Error> {
        if !self.measuring {
            return Err(Error::InvalidState {
                operation: Operation::StopMeasure,
                state: "stopped",
            });
        }
        self.runtime
            .call(Operation::StopMeasure, |driver| driver.stop(self.handle()))?;
        self.measuring = false;
        drop(self.image_registration.take());
        Ok(())
    }

    /// Forwards one software trigger to the SDK.
    pub fn soft_trigger(&mut self) -> Result<(), Error> {
        self.runtime.call(Operation::SoftTrigger, |driver| {
            driver.soft_trigger(self.handle())
        })
    }

    /// Returns one pull frame; `u32::MAX` selects the SDK's infinite wait.
    pub fn get_image(&mut self, timeout_ms: u32) -> Result<FrameRecord, Error> {
        self.runtime.call(Operation::GetImage, |driver| {
            driver.get_image(self.handle(), timeout_ms)
        })
    }

    pub fn register_exception_callback(
        &mut self,
        sink: ExceptionCallbackSink,
    ) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterExceptionCallback;
        let registration = CallbackRegistration::exception(sink);
        self.runtime.call(OPERATION, |driver| {
            driver.register_exception_callback(self.handle(), registration.cookie())
        })?;
        drop(self.exception_registration.replace(registration));
        Ok(())
    }

    /// Retires the exception cookie; a callback that already cloned its sink may finish.
    pub fn disable_exception_delivery(&mut self) {
        drop(self.exception_registration.take());
    }

    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        self.runtime.call(Operation::ClearDataBuffer, |driver| {
            driver.clear_buffer(self.handle())
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<ParameterRecord, Error> {
        let key = RuntimeCore::parameter_key(Operation::GetParam, key)?;
        self.runtime.call(Operation::GetParam, |driver| {
            driver.get_parameter(self.handle(), &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        let key = RuntimeCore::parameter_key(Operation::SetParam, key)?;
        self.runtime.call(Operation::SetParam, |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        let key = RuntimeCore::parameter_key(Operation::Execute, key)?;
        self.runtime.call(Operation::Execute, |driver| {
            driver.execute(self.handle(), &key)
        })
    }

    /// Starts a download and retains its native file names for the asynchronous transfer.
    pub fn download_file(
        &mut self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<(), Error> {
        self.begin_file_transfer(Operation::FileAccessRead, user_file_name, device_file_name)
    }

    /// Starts an upload and retains its native file names for the asynchronous transfer.
    pub fn upload_file(
        &mut self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        self.begin_file_transfer(Operation::FileAccessWrite, user_file_name, device_file_name)
    }

    /// Replaces retained file names only after the next transfer starts successfully.
    fn begin_file_transfer(
        &mut self,
        operation: Operation,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        let user_file_name = validated_file_name(operation, user_file_name)?;
        let device_file_name = validated_file_name(operation, device_file_name)?;
        let handle = self.handle();
        self.runtime.call(operation, |driver| match operation {
            Operation::FileAccessRead => {
                driver.file_access_read(handle, &user_file_name, &device_file_name)
            }
            Operation::FileAccessWrite => {
                driver.file_access_write(handle, &user_file_name, &device_file_name)
            }
            _ => unreachable!("begin_file_transfer accepts only file access operations"),
        })?;
        self.file_names = Some((user_file_name, device_file_name));
        Ok(())
    }

    /// Returns the current native file-transfer progress.
    pub fn file_transfer_progress(&mut self) -> Result<FileProgress, Error> {
        const OPERATION: Operation = Operation::GetFileAccessProgress;
        self.runtime.call(OPERATION, |driver| {
            driver.file_access_progress(self.handle())
        })
    }

    /// Stops acquisition when needed and closes the owned handle.
    ///
    /// The consumed owner calls native Close exactly once. Returning consumes the handle for every
    /// status, and every Stop or Close error is reported.
    pub fn close(mut self) -> Result<(), DeviceCleanupError> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), DeviceCleanupError> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let stop = if self.measuring {
            self.runtime
                .call(Operation::StopMeasure, |driver| driver.stop(handle))
                .err()
                .map(Box::new)
        } else {
            None
        };
        let close = self
            .runtime
            .cleanup_close_handle(handle)
            .err()
            .map(Box::new);
        self.measuring = false;
        drop(self.image_registration.take());
        drop(self.exception_registration.take());
        drop(self.file_names.take());

        if stop.is_none() && close.is_none() {
            Ok(())
        } else {
            Err(DeviceCleanupError { stop, close })
        }
    }

    fn handle(&self) -> Handle {
        self.handle.expect("a live device always has a handle")
    }

    fn require_stopped(&self, operation: Operation) -> Result<(), Error> {
        if self.measuring {
            Err(Error::InvalidState {
                operation,
                state: "measuring",
            })
        } else {
            Ok(())
        }
    }
}

fn validated_file_name(operation: Operation, bytes: &[u8]) -> Result<CString, Error> {
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        operation,
        kind: InvalidInput::InteriorNul,
    })
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
