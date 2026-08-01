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
    image_callback_attempted: bool,
    exception_callback_attempted: bool,
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
            image_callback_attempted: false,
            exception_callback_attempted: false,
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
            self.state = DeviceState::Faulted;
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
                self.state = DeviceState::Faulted;
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
                self.state = DeviceState::Faulted;
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
        self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<FileTransfer<'runtime>, FileTransferStartError<'runtime>> {
        self.begin_file_transfer(
            "MV3D_LP_FileAccessRead",
            FileTransferDirection::DeviceToHost,
            user_file_name,
            device_file_name,
        )
    }

    pub fn upload_file(
        self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'runtime>, FileTransferStartError<'runtime>> {
        self.begin_file_transfer(
            "MV3D_LP_FileAccessWrite",
            FileTransferDirection::HostToDevice,
            user_file_name,
            device_file_name,
        )
    }

    fn begin_file_transfer(
        mut self,
        operation: &'static str,
        direction: FileTransferDirection,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'runtime>, FileTransferStartError<'runtime>> {
        if let Err(source) = self.require_state(operation, &[DeviceState::Open]) {
            return Err(FileTransferStartError::rejected(source, self));
        }
        let user_file_name = match validated_file_name(operation, user_file_name) {
            Ok(name) => name,
            Err(source) => return Err(FileTransferStartError::rejected(source, self)),
        };
        let device_file_name = match validated_file_name(operation, device_file_name) {
            Ok(name) => name,
            Err(source) => return Err(FileTransferStartError::rejected(source, self)),
        };
        self.pending_transfer = Some(ActiveFileTransfer::new(user_file_name, device_file_name));

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
                Ok(FileTransfer {
                    device: self,
                    direction,
                    state: FileTransferState::Running,
                    _not_sync: PhantomData,
                })
            }
            Err(start) => {
                self.state = DeviceState::Faulted;
                let cleanup = self.cleanup().err();
                Err(FileTransferStartError::failed(start, cleanup))
            }
        }
    }

    fn file_transfer_progress(&mut self) -> Result<FileTransferStatus, Error> {
        self.require_state(
            "MV3D_LP_GetFileAccessProgress",
            &[DeviceState::Transferring],
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
            self.pending_transfer
                .take()
                .expect("active transfer retains its file names until completion is observed");
            self.state = DeviceState::Open;
            Ok(FileTransferStatus::Completed(progress))
        } else {
            Ok(FileTransferStatus::Running(progress))
        }
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

        if close.is_none() {
            self.pending_transfer.take();
        } else {
            // Close failure leaves the asynchronous pointer-retention contract unknown. Leaking
            // the active filename bundle is preferable to freeing memory the SDK may still read.
            if let Some(pending) = self.pending_transfer.take() {
                std::mem::forget(pending);
            }
        }

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
    last_completed: Option<u64>,
    last_total: Option<u64>,
}

impl ActiveFileTransfer {
    fn new(user_file_name: CString, device_file_name: CString) -> Self {
        Self {
            names: FileNameBundle::new(user_file_name, device_file_name),
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FileTransferState {
    Running,
    Completed(FileProgress),
}

/// An asynchronous file transfer that owns its device.
///
/// Unique ownership of the transfer may move between threads. It remains `!Sync`, so polling and
/// cleanup cannot run concurrently. Dropping it closes the owned device; after completion,
/// [`FileTransfer::try_into_device`] returns the device for reuse.
#[must_use = "dropping FileTransfer closes its owned device"]
pub struct FileTransfer<'runtime> {
    device: Device<'runtime>,
    direction: FileTransferDirection,
    state: FileTransferState,
    _not_sync: PhantomData<Cell<()>>,
}

impl<'runtime> FileTransfer<'runtime> {
    pub fn direction(&self) -> FileTransferDirection {
        self.direction
    }

    pub fn progress(&mut self) -> Result<FileTransferStatus, Error> {
        match self.state {
            FileTransferState::Running => {
                let status = self.device.file_transfer_progress()?;
                if let FileTransferStatus::Completed(progress) = status {
                    self.state = FileTransferState::Completed(progress);
                }
                Ok(status)
            }
            FileTransferState::Completed(progress) => Ok(FileTransferStatus::Completed(progress)),
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

    #[allow(
        clippy::result_large_err,
        reason = "the ownership-preserving API must return the unchanged active transfer"
    )]
    pub fn try_into_device(self) -> Result<(Device<'runtime>, FileProgress), Self> {
        if let FileTransferState::Completed(progress) = self.state {
            Ok((self.device, progress))
        } else {
            Err(self)
        }
    }

    pub fn close(self) -> Result<(), DeviceCleanupError> {
        self.device.close()
    }

    #[cfg(test)]
    pub(crate) fn retained_file_name_addresses_for_test(&self) -> Vec<(usize, usize)> {
        self.device.retained_file_name_addresses_for_test()
    }
}

enum FileTransferStartErrorKind<'runtime> {
    RejectedBeforeDriverEntry {
        source: Error,
        device: Device<'runtime>,
    },
    FailedAfterDriverEntry {
        start: Error,
        cleanup: Option<DeviceCleanupError>,
    },
}

/// A resource-owning failure to start a file transfer.
///
/// A rejection before entering the native Driver retains the device. Consume the error with
/// [`FileTransferStartError::into_rejected_device`] to recover it; otherwise dropping the error
/// closes that device. Once the Driver was entered, the device is closed immediately and cannot
/// be recovered.
#[must_use = "a rejected file transfer may contain a recoverable device"]
pub struct FileTransferStartError<'runtime> {
    kind: Box<FileTransferStartErrorKind<'runtime>>,
}

impl<'runtime> FileTransferStartError<'runtime> {
    fn rejected(source: Error, device: Device<'runtime>) -> Self {
        Self {
            kind: Box::new(FileTransferStartErrorKind::RejectedBeforeDriverEntry {
                source,
                device,
            }),
        }
    }

    fn failed(start: Error, cleanup: Option<DeviceCleanupError>) -> Self {
        Self {
            kind: Box::new(FileTransferStartErrorKind::FailedAfterDriverEntry { start, cleanup }),
        }
    }

    pub fn start_error(&self) -> &Error {
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, .. } => source,
            FileTransferStartErrorKind::FailedAfterDriverEntry { start, .. } => start,
        }
    }

    pub fn cleanup_error(&self) -> Option<&DeviceCleanupError> {
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { .. } => None,
            FileTransferStartErrorKind::FailedAfterDriverEntry { cleanup, .. } => cleanup.as_ref(),
        }
    }

    pub fn into_rejected_device(self) -> Result<(Error, Device<'runtime>), Self> {
        match *self.kind {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, device } => {
                Ok((source, device))
            }
            kind @ FileTransferStartErrorKind::FailedAfterDriverEntry { .. } => Err(Self {
                kind: Box::new(kind),
            }),
        }
    }

    #[doc(hidden)]
    pub fn into_failed_after_driver_entry(
        self,
    ) -> Result<(Error, Option<DeviceCleanupError>), Self> {
        match *self.kind {
            FileTransferStartErrorKind::FailedAfterDriverEntry { start, cleanup } => {
                Ok((start, cleanup))
            }
            kind @ FileTransferStartErrorKind::RejectedBeforeDriverEntry { .. } => Err(Self {
                kind: Box::new(kind),
            }),
        }
    }
}

impl fmt::Debug for FileTransferStartError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("FileTransferStartError");
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, .. } => debug
                .field("phase", &"rejected before Driver entry")
                .field("source", source),
            FileTransferStartErrorKind::FailedAfterDriverEntry { start, cleanup } => debug
                .field("phase", &"failed after Driver entry")
                .field("start", start)
                .field("cleanup", cleanup),
        };
        debug.finish()
    }
}

impl fmt::Display for FileTransferStartError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, .. } => write!(
                formatter,
                "file transfer was rejected before entering the Driver: {source}"
            ),
            FileTransferStartErrorKind::FailedAfterDriverEntry {
                start,
                cleanup: Some(cleanup),
            } => write!(
                formatter,
                "file transfer failed after entering the Driver ({start}); device cleanup also failed ({cleanup})"
            ),
            FileTransferStartErrorKind::FailedAfterDriverEntry {
                start,
                cleanup: None,
            } => write!(
                formatter,
                "file transfer failed after entering the Driver and the device was closed: {start}"
            ),
        }
    }
}

impl std::error::Error for FileTransferStartError<'_> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.start_error())
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
                *self.state = DeviceState::CallbackRetired;
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
