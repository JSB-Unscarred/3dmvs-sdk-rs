use std::ffi::CString;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::callback::{
    CallbackRegistration, CallbackStatsRecord, ExceptionCallbackSink, FrameCallbackSink,
};
use crate::driver::Handle;
use crate::error::{Error, InvalidInput};
use crate::file_transfer::{FileProgress, FileTransferDirection, FileTransferStatus};
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::Runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraState {
    Open,
    Measuring,
    CallbackMeasuring,
    Faulted,
    Transferring,
    CallbackRetired,
}

impl CameraState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Measuring => "measuring",
            Self::CallbackMeasuring => "callback measuring",
            Self::Faulted => "faulted",
            Self::Transferring => "transferring",
            Self::CallbackRetired => "callback retired",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupError {
    pub stop: Option<Box<Error>>,
    pub close: Option<Box<Error>>,
}

pub struct Camera<'runtime> {
    runtime: &'runtime Runtime,
    handle: Option<Handle>,
    state: CameraState,
    pending_transfer: Option<ActiveFileTransfer>,
    image_registration: Option<CallbackRegistration>,
    exception_registration: Option<CallbackRegistration>,
    image_callback_attempted: bool,
    exception_callback_attempted: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'runtime> Camera<'runtime> {
    pub(crate) fn new(runtime: &'runtime Runtime, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            state: CameraState::Open,
            pending_transfer: None,
            image_registration: None,
            exception_registration: None,
            image_callback_attempted: false,
            exception_callback_attempted: false,
            _not_send_or_sync: PhantomData,
        }
    }

    pub fn state(&self) -> CameraState {
        self.state
    }

    pub fn start(&mut self) -> Result<Measurement<'_>, Error> {
        self.require_state("MV3D_LP_StartMeasure", &[CameraState::Open])?;
        let result = self
            .runtime
            .call("MV3D_LP_StartMeasure", |driver| driver.start(self.handle()));
        match result {
            Ok(()) => {
                self.state = CameraState::Measuring;
                let handle = self.handle();
                Ok(Measurement {
                    runtime: self.runtime,
                    handle,
                    state: &mut self.state,
                    active: true,
                    _not_send_or_sync: PhantomData,
                })
            }
            Err(error) => {
                self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    pub fn start_callback(
        &mut self,
        sink: FrameCallbackSink,
    ) -> Result<CallbackMeasurement<'_>, Error> {
        const OPERATION: &str = "MV3D_LP_RegisterImageDataCallBack";
        self.require_state(OPERATION, &[CameraState::Open])?;
        if self.image_callback_attempted {
            return Err(Error::InvalidState {
                operation: OPERATION,
                state: "image callback registration already attempted",
            });
        }

        let mut registration = CallbackRegistration::image(sink)?;
        self.image_callback_attempted = true;
        let register = self.runtime.call(OPERATION, |driver| {
            driver.register_image_callback(self.handle(), registration.cookie())
        });
        if let Err(error) = register {
            registration.deactivate();
            self.state = CameraState::Faulted;
            return Err(error);
        }

        let start = self
            .runtime
            .call("MV3D_LP_StartMeasure", |driver| driver.start(self.handle()));
        match start {
            Ok(()) => {
                self.state = CameraState::CallbackMeasuring;
                let handle = self.handle();
                self.image_registration = Some(registration);
                Ok(CallbackMeasurement {
                    runtime: self.runtime,
                    handle,
                    state: &mut self.state,
                    registration: &mut self.image_registration,
                    active: true,
                    _not_send_or_sync: PhantomData,
                })
            }
            Err(error) => {
                registration.deactivate();
                self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    pub fn register_exception_callback(
        &mut self,
        sink: ExceptionCallbackSink,
    ) -> Result<(), Error> {
        const OPERATION: &str = "MV3D_LP_RegisterExceptionCallBack";
        self.require_state(OPERATION, &[CameraState::Open])?;
        if self.exception_callback_attempted {
            return Err(Error::InvalidState {
                operation: OPERATION,
                state: "exception callback registration already attempted",
            });
        }

        let mut registration = CallbackRegistration::exception(sink)?;
        self.exception_callback_attempted = true;
        let register = self.runtime.call(OPERATION, |driver| {
            driver.register_exception_callback(self.handle(), registration.cookie())
        });
        match register {
            Ok(()) => {
                self.exception_registration = Some(registration);
                Ok(())
            }
            Err(error) => {
                registration.deactivate();
                self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    pub fn exception_callback_stats(&self) -> Option<CallbackStatsRecord> {
        self.exception_registration
            .as_ref()
            .map(CallbackRegistration::stats)
    }

    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        self.require_state(
            "MV3D_LP_ClearDataBuffer",
            &[CameraState::Open, CameraState::Measuring],
        )?;
        self.runtime.call("MV3D_LP_ClearDataBuffer", |driver| {
            driver.clear_buffer(self.handle())
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<ParameterRecord, Error> {
        self.require_usable("MV3D_LP_GetParam")?;
        let key = Runtime::parameter_key("MV3D_LP_GetParam", key)?;
        self.runtime.call("MV3D_LP_GetParam", |driver| {
            driver.get_parameter(self.handle(), &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        self.require_usable("MV3D_LP_SetParam")?;
        if let ParameterValueRecord::String(value) = value {
            if value.len() > 255 {
                return Err(Error::InvalidInput {
                    operation: "MV3D_LP_SetParam",
                    kind: InvalidInput::TooLong {
                        actual: value.len(),
                        maximum: 255,
                    },
                });
            }
            if value.contains(&0) {
                return Err(Error::InvalidInput {
                    operation: "MV3D_LP_SetParam",
                    kind: InvalidInput::InteriorNul,
                });
            }
        }
        let key = Runtime::parameter_key("MV3D_LP_SetParam", key)?;
        self.runtime.call("MV3D_LP_SetParam", |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        self.require_usable("MV3D_LP_Execute")?;
        let key = Runtime::parameter_key("MV3D_LP_Execute", key)?;
        self.runtime.call("MV3D_LP_Execute", |driver| {
            driver.execute(self.handle(), &key)
        })
    }

    pub fn download_file<'camera>(
        &'camera mut self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<FileTransfer<'camera, 'runtime>, Error> {
        self.begin_file_transfer(
            "MV3D_LP_FileAccessRead",
            FileTransferDirection::DeviceToHost,
            user_file_name,
            device_file_name,
        )
    }

    pub fn upload_file<'camera>(
        &'camera mut self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'camera, 'runtime>, Error> {
        self.begin_file_transfer(
            "MV3D_LP_FileAccessWrite",
            FileTransferDirection::HostToDevice,
            user_file_name,
            device_file_name,
        )
    }

    pub fn active_file_transfer(&mut self) -> Option<FileTransfer<'_, 'runtime>> {
        if self.state == CameraState::Transferring && self.pending_transfer.is_some() {
            let direction = self
                .pending_transfer
                .as_ref()
                .expect("checked above")
                .direction;
            Some(FileTransfer {
                camera: self,
                direction,
            })
        } else {
            None
        }
    }

    fn begin_file_transfer<'camera>(
        &'camera mut self,
        operation: &'static str,
        direction: FileTransferDirection,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'camera, 'runtime>, Error> {
        self.require_state(operation, &[CameraState::Open])?;
        let user_file_name = validated_file_name(operation, user_file_name)?;
        let device_file_name = validated_file_name(operation, device_file_name)?;
        self.pending_transfer = Some(ActiveFileTransfer {
            user_file_name,
            device_file_name,
            direction,
            last_completed: None,
            last_total: None,
        });

        let pending = self
            .pending_transfer
            .as_ref()
            .expect("the transfer names were stored before the SDK call");
        let result = self.runtime.call(operation, |driver| match direction {
            FileTransferDirection::DeviceToHost => driver.file_access_read(
                self.handle(),
                &pending.user_file_name,
                &pending.device_file_name,
            ),
            FileTransferDirection::HostToDevice => driver.file_access_write(
                self.handle(),
                &pending.user_file_name,
                &pending.device_file_name,
            ),
        });

        match result {
            Ok(()) => {
                self.state = CameraState::Transferring;
                Ok(FileTransfer {
                    camera: self,
                    direction,
                })
            }
            Err(error) => {
                // A failed asynchronous start has undocumented partial-state semantics. Retain
                // both names and allow only CloseDevice, mirroring failed measurement starts.
                self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    fn file_transfer_progress(&mut self) -> Result<FileTransferStatus, Error> {
        self.require_state(
            "MV3D_LP_GetFileAccessProgress",
            &[CameraState::Transferring],
        )?;
        let raw = self
            .runtime
            .call("MV3D_LP_GetFileAccessProgress", |driver| {
                driver.file_access_progress(self.handle())
            })?;
        if raw.completed < 0 || raw.total < 0 {
            return Err(Error::ContractViolation {
                operation: "MV3D_LP_GetFileAccessProgress",
                kind: crate::error::ContractViolation::NegativeFileProgress {
                    completed: raw.completed,
                    total: raw.total,
                },
            });
        }
        let progress = FileProgress {
            completed: raw.completed as u64,
            total: raw.total as u64,
        };
        if progress.completed > progress.total {
            return Err(Error::ContractViolation {
                operation: "MV3D_LP_GetFileAccessProgress",
                kind: crate::error::ContractViolation::FileProgressExceedsTotal {
                    completed: progress.completed,
                    total: progress.total,
                },
            });
        }

        let pending = self
            .pending_transfer
            .as_mut()
            .expect("transferring state always retains its file names");
        if progress.total > 0 {
            if let Some(previous) = pending.last_total {
                if progress.total != previous {
                    return Err(Error::ContractViolation {
                        operation: "MV3D_LP_GetFileAccessProgress",
                        kind: crate::error::ContractViolation::FileProgressTotalChanged {
                            previous,
                            current: progress.total,
                        },
                    });
                }
            }
            pending.last_total = Some(progress.total);
        } else if let Some(previous) = pending.last_total {
            return Err(Error::ContractViolation {
                operation: "MV3D_LP_GetFileAccessProgress",
                kind: crate::error::ContractViolation::FileProgressTotalChanged {
                    previous,
                    current: 0,
                },
            });
        }
        if let Some(previous) = pending.last_completed {
            if progress.completed < previous {
                return Err(Error::ContractViolation {
                    operation: "MV3D_LP_GetFileAccessProgress",
                    kind: crate::error::ContractViolation::FileProgressRegressed {
                        previous,
                        current: progress.completed,
                    },
                });
            }
        }
        pending.last_completed = Some(progress.completed);

        if progress.total > 0 && progress.completed == progress.total {
            self.pending_transfer.take();
            self.state = CameraState::Open;
            Ok(FileTransferStatus::Completed(progress))
        } else {
            Ok(FileTransferStatus::Running(progress))
        }
    }

    pub fn close(mut self) -> Result<(), CleanupError> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), CleanupError> {
        if let Some(mut registration) = self.image_registration.take() {
            registration.deactivate();
        }
        if let Some(mut registration) = self.exception_registration.take() {
            registration.deactivate();
        }
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let stop = if matches!(
            self.state,
            CameraState::Measuring | CameraState::CallbackMeasuring
        ) || (self.state == CameraState::Faulted && self.pending_transfer.is_none())
        {
            self.runtime
                .cleanup_call("MV3D_LP_StopMeasure", |driver| driver.stop(handle))
                .err()
                .map(Box::new)
        } else {
            None
        };
        let close = self
            .runtime
            .cleanup_call("MV3D_LP_CloseDevice", |driver| driver.close(handle))
            .err()
            .map(Box::new);
        self.runtime.record_close_result(close.is_none());

        if close.is_none() {
            self.pending_transfer.take();
        } else if let Some(pending) = self.pending_transfer.take() {
            // Close failure leaves the asynchronous pointer-retention contract unknown. Leaking
            // two small C strings is preferable to letting the SDK observe freed memory.
            std::mem::forget(pending);
        }

        if stop.is_none() && close.is_none() {
            Ok(())
        } else {
            Err(CleanupError { stop, close })
        }
    }

    fn handle(&self) -> Handle {
        self.handle.expect("a live camera always has a handle")
    }

    fn require_usable(&self, operation: &'static str) -> Result<(), Error> {
        self.require_state(operation, &[CameraState::Open, CameraState::Measuring])
    }

    fn require_state(&self, operation: &'static str, allowed: &[CameraState]) -> Result<(), Error> {
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(Error::InvalidState {
                operation,
                state: self.state.as_str(),
            })
        }
    }
}

struct ActiveFileTransfer {
    user_file_name: CString,
    device_file_name: CString,
    direction: FileTransferDirection,
    last_completed: Option<u64>,
    last_total: Option<u64>,
}

/// An asynchronous file transfer borrowing its camera exclusively.
///
/// Dropping this guard does not cancel the transfer. The camera retains both
/// filenames, and [`Camera::active_file_transfer`] can resume progress polling.
#[must_use = "dropping FileTransfer does not cancel the device transfer"]
pub struct FileTransfer<'camera, 'runtime> {
    camera: &'camera mut Camera<'runtime>,
    direction: FileTransferDirection,
}

impl FileTransfer<'_, '_> {
    pub fn direction(&self) -> FileTransferDirection {
        self.direction
    }

    pub fn progress(&mut self) -> Result<FileTransferStatus, Error> {
        self.camera.file_transfer_progress()
    }
}

fn validated_file_name(operation: &'static str, bytes: &[u8]) -> Result<CString, Error> {
    if bytes.is_empty() {
        return Err(Error::InvalidInput {
            operation,
            kind: InvalidInput::Empty,
        });
    }
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        operation,
        kind: InvalidInput::InteriorNul,
    })
}

/// An active native-callback acquisition session.
///
/// This guard deliberately does not expose pull acquisition or buffer clearing. Its callback
/// registration is revoked and drained before `MV3D_LP_StopMeasure` is called.
pub struct CallbackMeasurement<'camera> {
    runtime: &'camera Runtime,
    handle: Handle,
    state: &'camera mut CameraState,
    registration: &'camera mut Option<CallbackRegistration>,
    active: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl CallbackMeasurement<'_> {
    pub fn state(&self) -> CameraState {
        *self.state
    }

    pub fn soft_trigger(&mut self) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_SoftTrigger")?;
        self.runtime.call("MV3D_LP_SoftTrigger", |driver| {
            driver.soft_trigger(self.handle)
        })
    }

    pub fn callback_stats(&self) -> CallbackStatsRecord {
        self.registration
            .as_ref()
            .map(CallbackRegistration::stats)
            .unwrap_or_default()
    }

    pub fn stop(mut self) -> Result<(), Error> {
        let result = self.stop_with(false);
        self.active = false;
        result
    }

    fn stop_with(&mut self, cleanup: bool) -> Result<(), Error> {
        self.deactivate_callback();
        self.require_measuring("MV3D_LP_StopMeasure")?;
        let result = if cleanup {
            self.runtime
                .cleanup_call("MV3D_LP_StopMeasure", |driver| driver.stop(self.handle))
        } else {
            self.runtime
                .call("MV3D_LP_StopMeasure", |driver| driver.stop(self.handle))
        };
        match result {
            Ok(()) => {
                *self.state = CameraState::CallbackRetired;
                Ok(())
            }
            Err(error) => {
                *self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    fn deactivate_callback(&mut self) {
        if let Some(mut registration) = self.registration.take() {
            registration.deactivate();
        }
    }

    fn require_measuring(&self, operation: &'static str) -> Result<(), Error> {
        if *self.state == CameraState::CallbackMeasuring {
            Ok(())
        } else {
            Err(Error::InvalidState {
                operation,
                state: self.state.as_str(),
            })
        }
    }
}

impl Drop for CallbackMeasurement<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.stop_with(true);
            self.active = false;
        } else {
            self.deactivate_callback();
        }
    }
}

/// A measuring session that exclusively borrows its camera state.
///
/// Dropping the value makes a best-effort call to `MV3D_LP_StopMeasure`.
pub struct Measurement<'camera> {
    runtime: &'camera Runtime,
    handle: Handle,
    state: &'camera mut CameraState,
    active: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Measurement<'_> {
    pub fn state(&self) -> CameraState {
        *self.state
    }

    pub fn soft_trigger(&mut self) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_SoftTrigger")?;
        self.runtime.call("MV3D_LP_SoftTrigger", |driver| {
            driver.soft_trigger(self.handle)
        })
    }

    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_ClearDataBuffer")?;
        self.runtime.call("MV3D_LP_ClearDataBuffer", |driver| {
            driver.clear_buffer(self.handle)
        })
    }

    pub fn get_image(&mut self, timeout_ms: u32) -> Result<FrameRecord, Error> {
        self.require_measuring("MV3D_LP_GetImage")?;
        if timeout_ms == u32::MAX {
            return Err(Error::InvalidInput {
                operation: "MV3D_LP_GetImage",
                kind: InvalidInput::TimeoutTooLong {
                    maximum_millis: u32::MAX - 1,
                    actual_millis: u128::from(timeout_ms),
                },
            });
        }
        self.runtime.call("MV3D_LP_GetImage", |driver| {
            driver.get_image(self.handle, timeout_ms)
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<ParameterRecord, Error> {
        self.require_measuring("MV3D_LP_GetParam")?;
        let key = Runtime::parameter_key("MV3D_LP_GetParam", key)?;
        self.runtime.call("MV3D_LP_GetParam", |driver| {
            driver.get_parameter(self.handle, &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_SetParam")?;
        validate_parameter_value("MV3D_LP_SetParam", value)?;
        let key = Runtime::parameter_key("MV3D_LP_SetParam", key)?;
        self.runtime.call("MV3D_LP_SetParam", |driver| {
            driver.set_parameter(self.handle, &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_Execute")?;
        let key = Runtime::parameter_key("MV3D_LP_Execute", key)?;
        self.runtime.call("MV3D_LP_Execute", |driver| {
            driver.execute(self.handle, &key)
        })
    }

    pub fn stop(mut self) -> Result<(), Error> {
        let result = self.stop_with(false);
        self.active = false;
        result
    }

    fn stop_with(&mut self, cleanup: bool) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_StopMeasure")?;
        let result = if cleanup {
            self.runtime
                .cleanup_call("MV3D_LP_StopMeasure", |driver| driver.stop(self.handle))
        } else {
            self.runtime
                .call("MV3D_LP_StopMeasure", |driver| driver.stop(self.handle))
        };
        match result {
            Ok(()) => {
                *self.state = CameraState::Open;
                Ok(())
            }
            Err(error) => {
                *self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    fn require_measuring(&self, operation: &'static str) -> Result<(), Error> {
        if *self.state == CameraState::Measuring {
            Ok(())
        } else {
            Err(Error::InvalidState {
                operation,
                state: self.state.as_str(),
            })
        }
    }
}

impl Drop for Measurement<'_> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.stop_with(true);
            self.active = false;
        }
    }
}

fn validate_parameter_value(
    operation: &'static str,
    value: &ParameterValueRecord,
) -> Result<(), Error> {
    if let ParameterValueRecord::String(value) = value {
        if value.len() > 255 {
            return Err(Error::InvalidInput {
                operation,
                kind: InvalidInput::TooLong {
                    actual: value.len(),
                    maximum: 255,
                },
            });
        }
        if value.contains(&0) {
            return Err(Error::InvalidInput {
                operation,
                kind: InvalidInput::InteriorNul,
            });
        }
    }
    Ok(())
}

impl Drop for Camera<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
