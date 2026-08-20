use std::sync::Arc;

use crate::callback::{CallbackRegistration, ExceptionCallback, ImageCallback};
use crate::cstr::c_string;
use crate::driver::Handle;
use crate::error::{Error, Operation};
use crate::file_transfer::FileProgress;
use crate::frame::Image;
use crate::parameter::{Parameter, ParameterValue};
use crate::runtime::RuntimeCore;

/// Opened device owning a lease on the initialized native session.
pub struct Device {
    runtime: Arc<RuntimeCore>,
    handle: Option<Handle>,
    acquisition: AcquisitionState,
    image_registration: Option<CallbackRegistration>,
    exception_registration: Option<CallbackRegistration>,
}

impl Device {
    pub(crate) fn new(runtime: Arc<RuntimeCore>, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            acquisition: AcquisitionState::Idle,
            image_registration: None,
            exception_registration: None,
        }
    }

    /// Registers an owned exception callback until it is replaced, disabled, or closed.
    pub fn register_exception_callback(&mut self, sink: ExceptionCallback) -> Result<(), Error> {
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

    /// Starts pull acquisition from idle, or callback acquisition after image registration.
    pub fn start(&mut self) -> Result<(), Error> {
        match self.acquisition {
            AcquisitionState::Idle => {
                self.runtime.call(Operation::StartMeasure, |driver| {
                    driver.start(self.handle())
                })?;
                self.acquisition = AcquisitionState::Pulling;
                Ok(())
            }
            AcquisitionState::CallbackStopped => {
                self.runtime.call(Operation::StartMeasure, |driver| {
                    driver.start(self.handle())
                })?;
                self.acquisition = AcquisitionState::CallbackRunning;
                Ok(())
            }
            _ => Err(Error::InvalidState {
                operation: Operation::StartMeasure,
                expected: "idle or callback registered and stopped",
                actual: self.acquisition.name(),
            }),
        }
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
            AcquisitionState::CallbackRunning => AcquisitionState::CallbackStopped,
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
    pub fn get_image(&mut self, timeout_ms: u32) -> Result<Image, Error> {
        self.runtime.call(Operation::GetImage, |driver| {
            driver.get_image(self.handle(), timeout_ms)
        })
    }

    /// Registers image callback delivery. Native registration binds this handle until Close.
    pub fn register_image_callback(&mut self, sink: ImageCallback) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterImageDataCallback;
        if matches!(self.acquisition, AcquisitionState::Pulling) {
            return Err(Error::InvalidState {
                operation: OPERATION,
                expected: "idle or callback registered",
                actual: self.acquisition.name(),
            });
        }

        let registration = CallbackRegistration::image(sink);
        self.runtime.call(OPERATION, |driver| {
            driver.register_image_callback(self.handle(), registration.cookie())
        })?;
        drop(self.image_registration.replace(registration));
        if matches!(self.acquisition, AcquisitionState::Idle) {
            self.acquisition = AcquisitionState::CallbackStopped;
        }
        Ok(())
    }

    /// Retires the image cookie; native registration remains until Close.
    pub fn disable_image_delivery(&mut self) {
        drop(self.image_registration.take());
    }

    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        self.runtime.call(Operation::ClearDataBuffer, |driver| {
            driver.clear_buffer(self.handle())
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<Parameter, Error> {
        let key = c_string("parameter key", key, None)?;
        self.runtime.call(Operation::GetParam, |driver| {
            driver.get_parameter(self.handle(), &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValue) -> Result<(), Error> {
        let key = c_string("parameter key", key, None)?;
        self.runtime.call(Operation::SetParam, |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        let key = c_string("command key", key, None)?;
        self.runtime.call(Operation::Execute, |driver| {
            driver.execute(self.handle(), &key)
        })
    }

    /// Starts a download. File names are passed for this native call only.
    pub fn download_file(
        &mut self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<(), Error> {
        self.file_transfer(Operation::FileAccessRead, user_file_name, device_file_name)
    }

    /// Starts an upload. File names are passed for this native call only.
    pub fn upload_file(
        &mut self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        self.file_transfer(Operation::FileAccessWrite, user_file_name, device_file_name)
    }

    fn file_transfer(
        &mut self,
        operation: Operation,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        let user_file_name = c_string("local file name", user_file_name, None)?;
        let device_file_name = c_string("device file name", device_file_name, None)?;
        let handle = self.handle();
        self.runtime.call(operation, |driver| match operation {
            Operation::FileAccessRead => {
                driver.file_access_read(handle, &user_file_name, &device_file_name)
            }
            Operation::FileAccessWrite => {
                driver.file_access_write(handle, &user_file_name, &device_file_name)
            }
            _ => unreachable!("file_transfer accepts only file access operations"),
        })
    }

    pub fn file_transfer_progress(&mut self) -> Result<FileProgress, Error> {
        const OPERATION: Operation = Operation::GetFileAccessProgress;
        self.runtime.call(OPERATION, |driver| {
            driver.file_access_progress(self.handle())
        })
    }

    /// Stops acquisition when needed and closes the owned handle.
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
            self.runtime.block_finalize();
        }
        self.acquisition = AcquisitionState::Idle;
        drop(self.image_registration.take());
        drop(self.exception_registration.take());

        cleanup_result(stop, close)
    }

    fn handle(&self) -> Handle {
        self.handle.expect("a live device always has a handle")
    }
}

/// Locally prevents acquisition modes from sharing one native handle.
enum AcquisitionState {
    Idle,
    Pulling,
    CallbackRunning,
    CallbackStopped,
}

impl AcquisitionState {
    fn needs_stop(&self) -> bool {
        matches!(self, Self::Pulling | Self::CallbackRunning)
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Pulling => "pull acquisition running",
            Self::CallbackRunning => "callback acquisition running",
            Self::CallbackStopped => "callback registered and stopped",
        }
    }
}

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
    use super::{AcquisitionState, cleanup_result};
    use crate::cstr::c_string;
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
                c_string(field, b"a\0b", None),
                Err(Error::InvalidInput {
                    field: actual,
                    violation: InputViolation::InteriorNul,
                }) if actual == field
            ));
        }
    }

    // 验证只有运行中的采集需要 Close 前 Stop。
    #[test]
    fn only_running_acquisitions_need_stop() {
        assert!(!AcquisitionState::Idle.needs_stop());
        assert!(AcquisitionState::Pulling.needs_stop());
        assert!(AcquisitionState::CallbackRunning.needs_stop());
        assert!(!AcquisitionState::CallbackStopped.needs_stop());
    }
}
