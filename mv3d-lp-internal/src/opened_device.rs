use std::cell::Cell;
#[cfg(test)]
use std::cell::RefCell;
use std::ffi::CString;
use std::fmt;
use std::marker::PhantomData;
#[cfg(test)]
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use crate::callback::{
    CallbackRegistration, CallbackStatsRecord, ExceptionCallbackSink, FrameCallbackSink,
};
use crate::driver::Handle;
use crate::error::{Error, InvalidInput};
use crate::file_transfer::{FileProgress, FileTransferDirection, FileTransferStatus};
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::RuntimeInner;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceState {
    Open,
    Measuring,
    CallbackMeasuring,
    Faulted,
    Transferring,
    CallbackRetired,
}

impl DeviceState {
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

pub struct Device<'runtime> {
    runtime: &'runtime RuntimeInner,
    handle: Option<Handle>,
    state: DeviceState,
    pending_transfer: Option<ActiveFileTransfer>,
    image_registration: Option<CallbackRegistration>,
    exception_registration: Option<CallbackRegistration>,
    _not_sync: PhantomData<Cell<()>>,
}

impl<'runtime> Device<'runtime> {
    pub(crate) fn new(runtime: &'runtime RuntimeInner, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            state: DeviceState::Open,
            pending_transfer: None,
            image_registration: None,
            exception_registration: None,
            _not_sync: PhantomData,
        }
    }

    pub fn state(&self) -> DeviceState {
        self.state
    }

    pub fn start(&mut self) -> Result<Measurement<'_>, Error> {
        self.require_state("MV3D_LP_StartMeasure", &[DeviceState::Open])?;
        self.runtime
            .call("MV3D_LP_StartMeasure", |driver| driver.start(self.handle()))?;
        self.state = DeviceState::Measuring;
        let handle = self.handle();
        Ok(Measurement {
            runtime: self.runtime,
            handle,
            state: &mut self.state,
            active: true,
            _not_sync: PhantomData,
        })
    }

    pub fn start_callback(
        &mut self,
        sink: FrameCallbackSink,
    ) -> Result<CallbackMeasurement<'_>, Error> {
        const OPERATION: &str = "MV3D_LP_RegisterImageDataCallBack";
        self.require_state(OPERATION, &[DeviceState::Open])?;

        let mut registration = CallbackRegistration::image(sink)?;
        let register = self.runtime.call(OPERATION, |driver| {
            driver.register_image_callback(self.handle(), registration.cookie())
        });
        if let Err(error) = register {
            registration.deactivate();
            return Err(error);
        }

        let start = self
            .runtime
            .call("MV3D_LP_StartMeasure", |driver| driver.start(self.handle()));
        match start {
            Ok(()) => {
                self.state = DeviceState::CallbackMeasuring;
                let handle = self.handle();
                self.image_registration = Some(registration);
                Ok(CallbackMeasurement {
                    runtime: self.runtime,
                    handle,
                    state: &mut self.state,
                    registration: &mut self.image_registration,
                    active: true,
                    _not_sync: PhantomData,
                })
            }
            Err(error) => {
                registration.deactivate();
                Err(error)
            }
        }
    }

    pub fn register_exception_callback(
        &mut self,
        sink: ExceptionCallbackSink,
    ) -> Result<(), Error> {
        const OPERATION: &str = "MV3D_LP_RegisterExceptionCallBack";
        self.require_state(OPERATION, &[DeviceState::Open])?;

        let mut registration = CallbackRegistration::exception(sink)?;
        let register = self.runtime.call(OPERATION, |driver| {
            driver.register_exception_callback(self.handle(), registration.cookie())
        });
        match register {
            Ok(()) => {
                if let Some(mut previous) = self.exception_registration.replace(registration) {
                    previous.deactivate();
                }
                Ok(())
            }
            Err(error) => {
                registration.deactivate();
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
            &[DeviceState::Open, DeviceState::Measuring],
        )?;
        self.runtime.call("MV3D_LP_ClearDataBuffer", |driver| {
            driver.clear_buffer(self.handle())
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<ParameterRecord, Error> {
        self.require_usable("MV3D_LP_GetParam")?;
        let key = RuntimeInner::parameter_key("MV3D_LP_GetParam", key)?;
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
        let key = RuntimeInner::parameter_key("MV3D_LP_SetParam", key)?;
        self.runtime.call("MV3D_LP_SetParam", |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        self.require_usable("MV3D_LP_Execute")?;
        let key = RuntimeInner::parameter_key("MV3D_LP_Execute", key)?;
        self.runtime.call("MV3D_LP_Execute", |driver| {
            driver.execute(self.handle(), &key)
        })
    }

    pub fn download_file(
        &mut self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<FileTransfer<'_>, Error> {
        self.begin_file_transfer(
            "MV3D_LP_FileAccessRead",
            FileTransferDirection::DeviceToHost,
            user_file_name,
            device_file_name,
        )
    }

    pub fn upload_file(
        &mut self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'_>, Error> {
        self.begin_file_transfer(
            "MV3D_LP_FileAccessWrite",
            FileTransferDirection::HostToDevice,
            user_file_name,
            device_file_name,
        )
    }

    fn begin_file_transfer(
        &mut self,
        operation: &'static str,
        direction: FileTransferDirection,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'_>, Error> {
        self.require_state(operation, &[DeviceState::Open])?;
        let user_file_name = validated_file_name(operation, user_file_name)?;
        let device_file_name = validated_file_name(operation, device_file_name)?;
        self.pending_transfer = Some(ActiveFileTransfer::new(
            user_file_name,
            device_file_name,
            direction,
        ));

        let pending = self
            .pending_transfer
            .as_ref()
            .expect("the transfer names were stored before the SDK call");
        let handle = self.handle();
        let result = self.runtime.call(operation, |driver| match direction {
            FileTransferDirection::DeviceToHost => driver.file_access_read(
                handle,
                &pending.names.user_file_name,
                &pending.names.device_file_name,
            ),
            FileTransferDirection::HostToDevice => driver.file_access_write(
                handle,
                &pending.names.user_file_name,
                &pending.names.device_file_name,
            ),
        });

        match result {
            Ok(()) => {
                self.state = DeviceState::Transferring;
                let handle = self.handle();
                Ok(FileTransfer {
                    runtime: self.runtime,
                    handle,
                    state: &mut self.state,
                    pending_transfer: &mut self.pending_transfer,
                    direction,
                    _not_sync: PhantomData,
                })
            }
            Err(start) => {
                self.state = DeviceState::Faulted;
                Err(start)
            }
        }
    }

    pub fn active_file_transfer(&mut self) -> Option<FileTransfer<'_>> {
        if self.state != DeviceState::Transferring {
            return None;
        }
        let direction = self.pending_transfer.as_ref()?.direction;
        let handle = self.handle();
        Some(FileTransfer {
            runtime: self.runtime,
            handle,
            state: &mut self.state,
            pending_transfer: &mut self.pending_transfer,
            direction,
            _not_sync: PhantomData,
        })
    }

    pub fn close(mut self) -> Result<(), DeviceCleanupError> {
        self.cleanup()
    }

    #[cfg(test)]
    pub(crate) fn retained_file_name_addresses_for_test(&self) -> Vec<(usize, usize)> {
        self.pending_transfer
            .iter()
            .map(|pending| pending.names.addresses())
            .collect()
    }

    fn cleanup(&mut self) -> Result<(), DeviceCleanupError> {
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
            DeviceState::Measuring | DeviceState::CallbackMeasuring
        ) || (self.state == DeviceState::Faulted && self.pending_transfer.is_none())
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
            .cleanup_close_handle(handle)
            .err()
            .map(Box::new);

        self.pending_transfer.take();

        if stop.is_none() && close.is_none() {
            Ok(())
        } else {
            Err(DeviceCleanupError { stop, close })
        }
    }

    fn handle(&self) -> Handle {
        self.handle.expect("a live device always has a handle")
    }

    fn require_usable(&self, operation: &'static str) -> Result<(), Error> {
        self.require_state(operation, &[DeviceState::Open, DeviceState::Measuring])
    }

    fn require_state(&self, operation: &'static str, allowed: &[DeviceState]) -> Result<(), Error> {
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
    names: FileNameBundle,
    direction: FileTransferDirection,
    last_completed: Option<u64>,
    last_total: Option<u64>,
}

impl ActiveFileTransfer {
    fn new(
        user_file_name: CString,
        device_file_name: CString,
        direction: FileTransferDirection,
    ) -> Self {
        Self {
            names: FileNameBundle::new(user_file_name, device_file_name),
            direction,
            last_completed: None,
            last_total: None,
        }
    }
}

struct FileNameBundle {
    user_file_name: CString,
    device_file_name: CString,
    #[cfg(test)]
    _lifetime: Arc<()>,
}

impl FileNameBundle {
    fn new(user_file_name: CString, device_file_name: CString) -> Self {
        #[cfg(test)]
        let lifetime = {
            let lifetime = Arc::new(());
            FILE_NAME_LIFETIMES.with(|lifetimes| {
                lifetimes.borrow_mut().push(Arc::downgrade(&lifetime));
            });
            lifetime
        };

        Self {
            user_file_name,
            device_file_name,
            #[cfg(test)]
            _lifetime: lifetime,
        }
    }

    #[cfg(test)]
    fn addresses(&self) -> (usize, usize) {
        (
            self.user_file_name.as_ptr() as usize,
            self.device_file_name.as_ptr() as usize,
        )
    }
}

#[cfg(test)]
std::thread_local! {
    static FILE_NAME_LIFETIMES: RefCell<Vec<Weak<()>>> = const { RefCell::new(Vec::new()) };
}

#[cfg(test)]
pub(crate) fn take_file_name_lifetimes_for_test() -> Vec<Weak<()>> {
    FILE_NAME_LIFETIMES.with(|lifetimes| std::mem::take(&mut *lifetimes.borrow_mut()))
}

/// An asynchronous file transfer that exclusively borrows its device state.
///
/// Unique ownership of the transfer may move between threads. It remains `!Sync`, so polling and
/// device access cannot run concurrently. Dropping this guard does not cancel the native transfer;
/// [`Device::active_file_transfer`] can resume progress polling.
#[must_use = "dropping FileTransfer does not cancel the device transfer"]
pub struct FileTransfer<'device> {
    runtime: &'device RuntimeInner,
    handle: Handle,
    state: &'device mut DeviceState,
    pending_transfer: &'device mut Option<ActiveFileTransfer>,
    direction: FileTransferDirection,
    _not_sync: PhantomData<Cell<()>>,
}

impl FileTransfer<'_> {
    pub fn direction(&self) -> FileTransferDirection {
        self.direction
    }

    pub fn progress(&mut self) -> Result<FileTransferStatus, Error> {
        const OPERATION: &str = "MV3D_LP_GetFileAccessProgress";

        if *self.state != DeviceState::Transferring {
            return Err(Error::InvalidState {
                operation: OPERATION,
                state: self.state.as_str(),
            });
        }

        let raw = self
            .runtime
            .call(OPERATION, |driver| driver.file_access_progress(self.handle))?;
        if raw.completed < 0 || raw.total < 0 {
            return Err(Error::ContractViolation {
                operation: OPERATION,
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
                operation: OPERATION,
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
                        operation: OPERATION,
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
                operation: OPERATION,
                kind: crate::error::ContractViolation::FileProgressTotalChanged {
                    previous,
                    current: 0,
                },
            });
        }
        if let Some(previous) = pending.last_completed {
            if progress.completed < previous {
                return Err(Error::ContractViolation {
                    operation: OPERATION,
                    kind: crate::error::ContractViolation::FileProgressRegressed {
                        previous,
                        current: progress.completed,
                    },
                });
            }
        }
        pending.last_completed = Some(progress.completed);

        if progress.total > 0 && progress.completed == progress.total {
            self.pending_transfer
                .take()
                .expect("active transfer retains its file names until completion is observed");
            *self.state = DeviceState::Open;
            Ok(FileTransferStatus::Completed(progress))
        } else {
            Ok(FileTransferStatus::Running(progress))
        }
    }

    pub fn wait_timeout(
        &mut self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<Option<FileProgress>, Error> {
        let started = Instant::now();
        let poll_interval = poll_interval.max(Duration::from_millis(1));
        loop {
            match self.progress()? {
                FileTransferStatus::Completed(progress) => return Ok(Some(progress)),
                FileTransferStatus::Running(_) => {}
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Ok(None);
            }
            let remaining = timeout.saturating_sub(elapsed);
            std::thread::sleep(poll_interval.min(remaining));
        }
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
pub struct CallbackMeasurement<'device> {
    runtime: &'device RuntimeInner,
    handle: Handle,
    state: &'device mut DeviceState,
    registration: &'device mut Option<CallbackRegistration>,
    active: bool,
    _not_sync: PhantomData<Cell<()>>,
}

impl CallbackMeasurement<'_> {
    pub fn state(&self) -> DeviceState {
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
                *self.state = DeviceState::Open;
                Ok(())
            }
            Err(error) => {
                *self.state = DeviceState::Faulted;
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
        if *self.state == DeviceState::CallbackMeasuring {
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

/// A measuring session that exclusively borrows its device state.
///
/// Dropping the value makes a best-effort call to `MV3D_LP_StopMeasure`.
pub struct Measurement<'device> {
    runtime: &'device RuntimeInner,
    handle: Handle,
    state: &'device mut DeviceState,
    active: bool,
    _not_sync: PhantomData<Cell<()>>,
}

impl Measurement<'_> {
    pub fn state(&self) -> DeviceState {
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
        let key = RuntimeInner::parameter_key("MV3D_LP_GetParam", key)?;
        self.runtime.call("MV3D_LP_GetParam", |driver| {
            driver.get_parameter(self.handle, &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_SetParam")?;
        validate_parameter_value("MV3D_LP_SetParam", value)?;
        let key = RuntimeInner::parameter_key("MV3D_LP_SetParam", key)?;
        self.runtime.call("MV3D_LP_SetParam", |driver| {
            driver.set_parameter(self.handle, &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        self.require_measuring("MV3D_LP_Execute")?;
        let key = RuntimeInner::parameter_key("MV3D_LP_Execute", key)?;
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
                *self.state = DeviceState::Open;
                Ok(())
            }
            Err(error) => {
                *self.state = DeviceState::Faulted;
                Err(error)
            }
        }
    }

    fn require_measuring(&self, operation: &'static str) -> Result<(), Error> {
        if *self.state == DeviceState::Measuring {
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

impl Drop for Device<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
