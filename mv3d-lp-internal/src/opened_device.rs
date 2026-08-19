use std::ffi::CString;
use std::sync::Arc;

use crate::callback::{CallbackRegistration, ExceptionCallbackSink, FrameCallbackSink};
use crate::driver::Handle;
use crate::error::{Error, InputViolation, Operation};
use crate::file_transfer::FileProgress;
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::RuntimeCore;

/// Opened device owning a lease on the initialized native session.
pub struct Device {
    runtime: Arc<RuntimeCore>,
    handle: Option<Handle>,
    acquisition: AcquisitionState,
    exception_registration: Option<CallbackRegistration>,
    // Successful asynchronous FileAccess calls may retain every submitted name until Close.
    file_names: Vec<(CString, CString)>,
}

impl Device {
    pub(crate) fn new(runtime: Arc<RuntimeCore>, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            acquisition: AcquisitionState::Idle,
            exception_registration: None,
            file_names: Vec::new(),
        }
    }

    /// Starts pull acquisition only from the idle state.
    pub fn start(&mut self) -> Result<(), Error> {
        self.acquisition.require_idle(Operation::StartMeasure)?;
        self.runtime.call(Operation::StartMeasure, |driver| {
            driver.start(self.handle())
        })?;
        self.acquisition = AcquisitionState::Pulling;
        Ok(())
    }

    /// Registers callback acquisition and retains its cookie through Close.
    pub fn start_callback(&mut self, sink: FrameCallbackSink) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterImageDataCallback;
        self.acquisition.require_idle(OPERATION)?;

        let registration = CallbackRegistration::image(sink);
        self.runtime.call(OPERATION, |driver| {
            driver.register_image_callback(self.handle(), registration.cookie())
        })?;
        // Register has no native unregister API, so the device remains callback-bound even when
        // the following Start fails.
        self.acquisition = AcquisitionState::CallbackStopped(registration);
        self.runtime.call(Operation::StartMeasure, |driver| {
            driver.start(self.handle())
        })?;
        let AcquisitionState::CallbackStopped(registration) =
            std::mem::replace(&mut self.acquisition, AcquisitionState::Idle)
        else {
            unreachable!("callback registration was saved before Start")
        };
        self.acquisition = AcquisitionState::CallbackRunning(registration);
        Ok(())
    }

    /// Stops active acquisition; callback registration remains valid through Close.
    pub fn stop(&mut self) -> Result<(), Error> {
        if !self.acquisition.needs_stop() {
            return Err(Error::InvalidState {
                operation: Operation::StopMeasure,
                expected: "pull or callback acquisition running",
                actual: self.acquisition.name(),
            });
        }
        self.runtime
            .call(Operation::StopMeasure, |driver| driver.stop(self.handle()))?;
        self.acquisition = match std::mem::replace(&mut self.acquisition, AcquisitionState::Idle) {
            AcquisitionState::Pulling => AcquisitionState::Idle,
            AcquisitionState::CallbackRunning(registration) => {
                AcquisitionState::CallbackStopped(registration)
            }
            _ => unreachable!("only a running acquisition reaches native Stop"),
        };
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
        let key = validated_c_string("parameter key", key)?;
        self.runtime.call(Operation::GetParam, |driver| {
            driver.get_parameter(self.handle(), &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        let key = validated_c_string("parameter key", key)?;
        self.runtime.call(Operation::SetParam, |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        let key = validated_c_string("command key", key)?;
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

    /// Retains every successfully submitted name pair until device cleanup.
    fn begin_file_transfer(
        &mut self,
        operation: Operation,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        let user_file_name = validated_c_string("local file name", user_file_name)?;
        let device_file_name = validated_c_string("device file name", device_file_name)?;
        // Reserve before native success so retaining its borrowed inputs cannot allocate afterward.
        self.file_names.reserve(1);
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
        self.file_names.push((user_file_name, device_file_name));
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
    pub fn close(mut self) -> Result<(), Error> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), Error> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let stop = if self.acquisition.needs_stop() {
            self.runtime
                .call(Operation::StopMeasure, |driver| driver.stop(handle))
                .err()
        } else {
            None
        };
        let close = self
            .runtime
            .call(Operation::CloseDevice, |driver| driver.close(handle))
            .err();

        if close.is_some() {
            // A failed Close makes Finalize unsafe and may leave native FileAccess borrows alive.
            self.runtime.block_finalize();
            std::mem::forget(std::mem::take(&mut self.file_names));
        } else {
            drop(std::mem::take(&mut self.file_names));
        }
        // Registration may outlive Stop, so opaque cookies are revoked after the sole Close call.
        self.acquisition = AcquisitionState::Idle;
        drop(self.exception_registration.take());

        cleanup_result(stop, close)
    }

    fn handle(&self) -> Handle {
        self.handle.expect("a live device always has a handle")
    }
}

/// Locally prevents acquisition modes from sharing one native handle or callback slot.
enum AcquisitionState {
    Idle,
    Pulling,
    CallbackRunning(CallbackRegistration),
    CallbackStopped(CallbackRegistration),
}

impl AcquisitionState {
    /// Rejects a second acquisition mode on the same handle or callback slot.
    fn require_idle(&self, operation: Operation) -> Result<(), Error> {
        if matches!(self, Self::Idle) {
            Ok(())
        } else {
            Err(Error::InvalidState {
                operation,
                expected: "idle",
                actual: self.name(),
            })
        }
    }

    /// Reports whether cleanup owes the native handle one Stop call.
    fn needs_stop(&self) -> bool {
        matches!(self, Self::Pulling | Self::CallbackRunning(_))
    }

    /// Supplies a stable description for public state errors.
    fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Pulling => "pull acquisition running",
            Self::CallbackRunning(_) => "callback acquisition running",
            Self::CallbackStopped(_) => "callback registered and stopped",
        }
    }
}

/// Owns one native string and rejects interior NUL bytes before the FFI call.
fn validated_c_string(field: &'static str, bytes: &[u8]) -> Result<CString, Error> {
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        field,
        violation: InputViolation::InteriorNul,
    })
}

/// Returns one cleanup error unchanged and aggregates only simultaneous failures.
fn cleanup_result(stop: Option<Error>, close: Option<Error>) -> Result<(), Error> {
    match (stop, close) {
        (None, None) => Ok(()),
        (Some(error), None) | (None, Some(error)) => Err(error),
        (Some(stop), Some(close)) => Err(Error::DeviceCleanup {
            stop: Box::new(stop),
            close: Box::new(close),
        }),
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{AcquisitionState, cleanup_result, validated_c_string};
    use crate::callback::CallbackRegistration;
    use crate::error::{Error, InputViolation, Operation, SdkError, StatusCode};

    // 验证清理只在 Stop 与 Close 同时失败时引入聚合错误。
    #[test]
    fn cleanup_returns_single_errors_unchanged() {
        let stop = Error::Sdk(SdkError::new(
            Operation::StopMeasure,
            StatusCode::RESOURCE_ERROR,
        ));
        let close = Error::Sdk(SdkError::new(
            Operation::CloseDevice,
            StatusCode::INVALID_HANDLE,
        ));

        assert_eq!(cleanup_result(None, None), Ok(()));
        assert_eq!(cleanup_result(Some(stop.clone()), None), Err(stop.clone()));
        assert_eq!(
            cleanup_result(None, Some(close.clone())),
            Err(close.clone())
        );
        assert_eq!(
            cleanup_result(Some(stop.clone()), Some(close.clone())),
            Err(Error::DeviceCleanup {
                stop: Box::new(stop),
                close: Box::new(close),
            })
        );
    }

    // 验证同一 FileAccess 调用的两个字符串错误仍能定位到具体参数。
    #[test]
    fn file_name_input_errors_preserve_the_field() {
        for field in ["local file name", "device file name"] {
            assert!(matches!(
                validated_c_string(field, b"a\0b"),
                Err(Error::InvalidInput {
                    field: actual,
                    violation: InputViolation::InteriorNul,
                }) if actual == field
            ));
        }
    }

    // 验证 callback 注册后即绑定到 Close，停止态不能切换采集模式。
    #[test]
    fn stopped_callback_registration_is_terminal() {
        let state =
            AcquisitionState::CallbackStopped(CallbackRegistration::image(Arc::new(|_| true)));

        assert!(!state.needs_stop());
        for operation in [
            Operation::StartMeasure,
            Operation::RegisterImageDataCallback,
        ] {
            assert!(matches!(
                state.require_idle(operation),
                Err(Error::InvalidState {
                    operation: actual,
                    ..
                }) if actual == operation
            ));
        }
    }
}
