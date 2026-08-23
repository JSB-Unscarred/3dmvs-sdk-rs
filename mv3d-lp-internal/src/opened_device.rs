use std::ffi::CString;
use std::sync::Arc;

use crate::callback::{CallbackRegistration, ExceptionCallback, ImageCallback};
use crate::cstr::c_string;
use crate::driver::Handle;
use crate::error::{Error, Operation};
use crate::ffi::NativeDriver;
use crate::file_transfer::FileProgress;
use crate::frame::Image;
use crate::parameter::{Parameter, ParameterValue};
use crate::runtime::{RuntimeCore, call_native};

/// Opened device owning a lease on the initialized native session.
pub struct Device {
    runtime: Arc<RuntimeCore>,
    handle: Option<Handle>,
    acquisition: AcquisitionState,
    image_registration: Option<CallbackRegistration>,
    exception_registration: Option<CallbackRegistration>,
}

impl Device {
    pub(crate) const fn new(runtime: Arc<RuntimeCore>, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            acquisition: AcquisitionState::Idle,
            image_registration: None,
            exception_registration: None,
        }
    }

    /// Registers an owned exception callback until it is replaced, disabled, or closed.
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn register_exception_callback(&mut self, sink: ExceptionCallback) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterExceptionCallback;
        let registration = CallbackRegistration::exception(sink);
        call_native(OPERATION, || {
            NativeDriver::register_exception_callback(self.handle(), registration.cookie())
        })?;
        drop(self.exception_registration.replace(registration));
        Ok(())
    }

    /// Retires the exception cookie; a callback that already cloned its sink may finish.
    pub fn disable_exception_delivery(&mut self) {
        drop(self.exception_registration.take());
    }

    /// Starts pull acquisition from idle, or callback acquisition after image registration.
    ///
    /// 次态先于 native 调用算出；调用失败时状态原样保留。
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn start(&mut self) -> Result<(), Error> {
        let next = match self.acquisition {
            AcquisitionState::Idle => AcquisitionState::Pulling,
            AcquisitionState::CallbackStopped => AcquisitionState::CallbackRunning,
            _ => {
                return Err(Error::InvalidState {
                    operation: Operation::StartMeasure,
                    expected: "idle or callback registered and stopped",
                    actual: self.acquisition.name(),
                });
            }
        };
        call_native(Operation::StartMeasure, || {
            NativeDriver::start(self.handle())
        })?;
        self.acquisition = next;
        Ok(())
    }

    /// Stops active acquisition; callback registration remains valid through Close.
    ///
    /// 次态先于 native 调用算出；调用失败时状态原样保留。
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn stop(&mut self) -> Result<(), Error> {
        let next = match self.acquisition {
            AcquisitionState::Pulling => AcquisitionState::Idle,
            AcquisitionState::CallbackRunning => AcquisitionState::CallbackStopped,
            _ => {
                return Err(Error::InvalidState {
                    operation: Operation::StopMeasure,
                    expected: "pull or callback acquisition running",
                    actual: self.acquisition.name(),
                });
            }
        };
        call_native(Operation::StopMeasure, || NativeDriver::stop(self.handle()))?;
        self.acquisition = next;
        Ok(())
    }

    /// Forwards one software trigger to the SDK.
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn soft_trigger(&mut self) -> Result<(), Error> {
        call_native(Operation::SoftTrigger, || {
            NativeDriver::soft_trigger(self.handle())
        })
    }

    /// Returns one pull frame; `u32::MAX` selects the SDK's infinite wait.
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`]。
    pub fn get_image(&mut self, timeout_ms: u32) -> Result<Image, Error> {
        call_native(Operation::GetImage, || {
            NativeDriver::get_image(self.handle(), timeout_ms)
        })
    }

    /// Blocks for one pull frame; the SDK's infinite-wait sentinel stays encoded here only.
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`]。
    pub fn get_image_blocking(&mut self) -> Result<Image, Error> {
        self.get_image(u32::MAX)
    }

    /// Registers image callback delivery. Native registration binds this handle until Close.
    ///
    /// # Errors
    ///
    /// 当前采集状态不允许该操作时返回 [`Error::InvalidState`]。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
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
        call_native(OPERATION, || {
            NativeDriver::register_image_callback(self.handle(), registration.cookie())
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

    /// Discards buffered frames. 允许调用的状态待厂商确认，因此不加本地状态校验。
    ///
    /// # Errors
    ///
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        call_native(Operation::ClearDataBuffer, || {
            NativeDriver::clear_buffer(self.handle())
        })
    }

    /// Reads one parameter. Node Name 只在本次 native 调用期间以 C 字符串传入。
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`]，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`]。
    pub fn get_parameter(&mut self, key: &[u8]) -> Result<Parameter, Error> {
        let key = c_string("parameter key", key)?;
        call_native(Operation::GetParam, || {
            NativeDriver::get_parameter(self.handle(), &key)
        })
    }

    /// Writes one parameter. Node Name 只在本次 native 调用期间以 C 字符串传入。
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`]，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValue) -> Result<(), Error> {
        let key = c_string("parameter key", key)?;
        call_native(Operation::SetParam, || {
            NativeDriver::set_parameter(self.handle(), &key, value)
        })
    }

    /// Executes one command. Command Node Name 只在本次 native 调用期间以 C 字符串传入。
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`]，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        let key = c_string("command key", key)?;
        call_native(Operation::Execute, || {
            NativeDriver::execute(self.handle(), &key)
        })
    }

    /// Starts a download. File names are passed for this native call only.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`]，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn download_file(
        &mut self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<(), Error> {
        let (user, device) = file_names(user_file_name, device_file_name)?;
        let handle = self.handle();
        call_native(Operation::FileAccessRead, || {
            NativeDriver::file_access_read(handle, &user, &device)
        })
    }

    /// Starts an upload. File names are passed for this native call only.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`]，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn upload_file(
        &mut self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        let (user, device) = file_names(user_file_name, device_file_name)?;
        let handle = self.handle();
        call_native(Operation::FileAccessWrite, || {
            NativeDriver::file_access_write(handle, &user, &device)
        })
    }

    /// Copies one progress snapshot without interpreting completion.
    ///
    /// # Errors
    ///
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`]。
    pub fn file_transfer_progress(&mut self) -> Result<FileProgress, Error> {
        const OPERATION: Operation = Operation::GetFileAccessProgress;
        call_native(OPERATION, || {
            NativeDriver::file_access_progress(self.handle())
        })
    }

    /// Stops acquisition when needed and closes the owned handle.
    ///
    /// # Errors
    ///
    /// Stop 与 Close 双双失败时返回 [`Error::DeviceCleanup`]，两个错误都保留；
    /// 单边失败时原样返回该错误。
    /// SDK 调用失败时返回 [`Error::Sdk`]；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`]。
    pub fn close(mut self) -> Result<(), Error> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), Error> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let stop = if self.acquisition.needs_stop() {
            call_native(Operation::StopMeasure, || NativeDriver::stop(handle)).err()
        } else {
            None
        };
        let close = call_native(Operation::CloseDevice, || NativeDriver::close(handle)).err();

        if close.is_some() {
            self.runtime.block_finalize();
        }
        // cleanup 后 Device 必然析构，registration 字段随 drop 在 Close 之后退休 cookie。

        cleanup_result(stop, close)
    }

    const fn handle(&self) -> Handle {
        self.handle.expect("a live device always has a handle")
    }
}

/// Locally prevents acquisition modes from sharing one native handle.
#[derive(Clone, Copy)]
enum AcquisitionState {
    Idle,
    Pulling,
    CallbackRunning,
    CallbackStopped,
}

impl AcquisitionState {
    const fn needs_stop(self) -> bool {
        matches!(self, Self::Pulling | Self::CallbackRunning)
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Pulling => "pull acquisition running",
            Self::CallbackRunning => "callback acquisition running",
            Self::CallbackStopped => "callback registered and stopped",
        }
    }
}

/// 两个 `FileAccess` 接口共用的字符串前处理；错误保留具体参数名。
fn file_names(user: &[u8], device: &[u8]) -> Result<(CString, CString), Error> {
    Ok((
        c_string("local file name", user)?,
        c_string("device file name", device)?,
    ))
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
                c_string(field, b"a\0b"),
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
