#[cfg(test)]
use std::cell::RefCell;
use std::ffi::CString;
use std::fmt;
use std::sync::Arc;
#[cfg(test)]
use std::sync::Weak;
use std::time::{Duration, Instant};

use crate::callback::{
    CallbackRegistration, CallbackStatsRecord, ExceptionCallbackSink, FrameCallbackSink,
};
use crate::driver::Handle;
use crate::error::{Error, InvalidInput, Operation};
use crate::file_transfer::{FileProgress, FileTransferStatus};
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::RuntimeCore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceState {
    Open,
    Measuring,
    CallbackMeasuring,
    Faulted,
    Transferring,
}

impl DeviceState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Measuring => "measuring",
            Self::CallbackMeasuring => "callback measuring",
            Self::Faulted => "faulted",
            Self::Transferring => "transferring",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FaultOrigin {
    AcquisitionStopUncertain,
    FileTransferStartUncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DevicePhase {
    Open,
    Measuring,
    CallbackMeasuring,
    Faulted(FaultOrigin),
    Transferring,
}

impl DevicePhase {
    const fn observable(self) -> DeviceState {
        match self {
            Self::Open => DeviceState::Open,
            Self::Measuring => DeviceState::Measuring,
            Self::CallbackMeasuring => DeviceState::CallbackMeasuring,
            Self::Faulted(_) => DeviceState::Faulted,
            Self::Transferring => DeviceState::Transferring,
        }
    }

    fn as_str(self) -> &'static str {
        self.observable().as_str()
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

/// Opened device owning a lease on the initialized native session.
pub struct Device {
    runtime: Arc<RuntimeCore>,
    handle: Option<Handle>,
    state: DevicePhase,
    pending_transfer: Option<FileNameBundle>,
    image_registration: Option<CallbackRegistration>,
    exception_registration: Option<CallbackRegistration>,
}

impl Device {
    pub(crate) fn new(runtime: Arc<RuntimeCore>, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            state: DevicePhase::Open,
            pending_transfer: None,
            image_registration: None,
            exception_registration: None,
        }
    }

    pub fn state(&self) -> DeviceState {
        self.state.observable()
    }

    /// Starts pull acquisition while keeping the session state on the device.
    pub fn start(&mut self) -> Result<(), Error> {
        self.require_state(Operation::StartMeasure, &[DeviceState::Open])?;
        self.runtime.call(Operation::StartMeasure, |driver| {
            driver.start(self.handle())
        })?;
        self.state = DevicePhase::Measuring;
        Ok(())
    }

    /// Starts callback acquisition and retains the registration until stop or close.
    pub fn start_callback(&mut self, sink: FrameCallbackSink) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterImageDataCallback;
        self.require_state(OPERATION, &[DeviceState::Open])?;

        let registration = CallbackRegistration::image(sink)?;
        self.runtime.call(OPERATION, |driver| {
            driver.register_image_callback(self.handle(), registration.cookie())
        })?;
        self.runtime.call(Operation::StartMeasure, |driver| {
            driver.start(self.handle())
        })?;
        self.image_registration = Some(registration);
        self.state = DevicePhase::CallbackMeasuring;
        Ok(())
    }

    /// Stops the active acquisition; callback delivery is revoked and drained first.
    pub fn stop(&mut self) -> Result<(), Error> {
        self.require_state(
            Operation::StopMeasure,
            &[DeviceState::Measuring, DeviceState::CallbackMeasuring],
        )?;

        drop(self.image_registration.take());
        let result = self
            .runtime
            .call(Operation::StopMeasure, |driver| driver.stop(self.handle()));
        match result {
            Ok(()) => {
                self.state = DevicePhase::Open;
                Ok(())
            }
            Err(error) => {
                self.state = DevicePhase::Faulted(FaultOrigin::AcquisitionStopUncertain);
                Err(error)
            }
        }
    }

    /// Issues a software trigger only while pull or callback acquisition is active.
    pub fn soft_trigger(&mut self) -> Result<(), Error> {
        self.require_state(
            Operation::SoftTrigger,
            &[DeviceState::Measuring, DeviceState::CallbackMeasuring],
        )?;
        self.runtime.call(Operation::SoftTrigger, |driver| {
            driver.soft_trigger(self.handle())
        })
    }

    /// Returns one pull frame; callback acquisition uses its registered sink instead.
    pub fn get_image(&mut self, timeout_ms: u32) -> Result<FrameRecord, Error> {
        self.require_state(Operation::GetImage, &[DeviceState::Measuring])?;
        if timeout_ms == u32::MAX {
            return Err(Error::InvalidInput {
                operation: Operation::GetImage,
                kind: InvalidInput::TimeoutTooLong {
                    maximum_millis: u32::MAX - 1,
                    actual_millis: u128::from(timeout_ms),
                },
            });
        }
        self.runtime.call(Operation::GetImage, |driver| {
            driver.get_image(self.handle(), timeout_ms)
        })
    }

    /// Returns callback counters while callback acquisition owns a live registration.
    pub fn image_callback_stats(&self) -> Option<CallbackStatsRecord> {
        self.image_registration
            .as_ref()
            .map(CallbackRegistration::stats)
    }

    pub fn register_exception_callback(
        &mut self,
        sink: ExceptionCallbackSink,
    ) -> Result<(), Error> {
        const OPERATION: Operation = Operation::RegisterExceptionCallback;
        self.require_state(OPERATION, &[DeviceState::Open])?;

        let registration = CallbackRegistration::exception(sink)?;
        self.runtime.call(OPERATION, |driver| {
            driver.register_exception_callback(self.handle(), registration.cookie())
        })?;
        drop(self.exception_registration.replace(registration));
        Ok(())
    }

    pub fn exception_callback_stats(&self) -> Option<CallbackStatsRecord> {
        self.exception_registration
            .as_ref()
            .map(CallbackRegistration::stats)
    }

    /// Revokes exception delivery and drains callbacks already using the sink.
    pub fn disable_exception_delivery(&mut self) {
        drop(self.exception_registration.take());
    }

    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        self.require_state(
            Operation::ClearDataBuffer,
            &[DeviceState::Open, DeviceState::Measuring],
        )?;
        self.runtime.call(Operation::ClearDataBuffer, |driver| {
            driver.clear_buffer(self.handle())
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<ParameterRecord, Error> {
        self.require_usable(Operation::GetParam)?;
        let key = RuntimeCore::parameter_key(Operation::GetParam, key)?;
        self.runtime.call(Operation::GetParam, |driver| {
            driver.get_parameter(self.handle(), &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        self.require_usable(Operation::SetParam)?;
        validate_parameter_value(Operation::SetParam, value)?;
        let key = RuntimeCore::parameter_key(Operation::SetParam, key)?;
        self.runtime.call(Operation::SetParam, |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        self.require_usable(Operation::Execute)?;
        let key = RuntimeCore::parameter_key(Operation::Execute, key)?;
        self.runtime.call(Operation::Execute, |driver| {
            driver.execute(self.handle(), &key)
        })
    }

    /// Starts a download and retains both names until completion or device close.
    pub fn download_file(
        &mut self,
        device_file_name: &[u8],
        user_file_name: &[u8],
    ) -> Result<(), Error> {
        self.begin_file_transfer(Operation::FileAccessRead, user_file_name, device_file_name)
    }

    /// Starts an upload and retains both names until completion or device close.
    pub fn upload_file(
        &mut self,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        self.begin_file_transfer(Operation::FileAccessWrite, user_file_name, device_file_name)
    }

    /// Stores native filename pointers before entering the asynchronous SDK call.
    fn begin_file_transfer(
        &mut self,
        operation: Operation,
        user_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<(), Error> {
        self.require_state(operation, &[DeviceState::Open])?;
        let user_file_name = validated_file_name(operation, user_file_name)?;
        let device_file_name = validated_file_name(operation, device_file_name)?;
        self.pending_transfer = Some(FileNameBundle::new(user_file_name, device_file_name));

        let pending = self
            .pending_transfer
            .as_ref()
            .expect("the transfer names were stored before the SDK call");
        let handle = self.handle();
        let result = self.runtime.call(operation, |driver| match operation {
            Operation::FileAccessRead => {
                driver.file_access_read(handle, &pending.user_file_name, &pending.device_file_name)
            }
            Operation::FileAccessWrite => {
                driver.file_access_write(handle, &pending.user_file_name, &pending.device_file_name)
            }
            _ => unreachable!("begin_file_transfer accepts only file access operations"),
        });

        match result {
            Ok(()) => {
                self.state = DevicePhase::Transferring;
                Ok(())
            }
            Err(start) => {
                self.state = DevicePhase::Faulted(FaultOrigin::FileTransferStartUncertain);
                Err(start)
            }
        }
    }

    /// Returns the current native file-transfer progress.
    pub fn file_transfer_progress(&mut self) -> Result<FileTransferStatus, Error> {
        const OPERATION: Operation = Operation::GetFileAccessProgress;
        self.require_state(OPERATION, &[DeviceState::Transferring])?;

        let raw = self.runtime.call(OPERATION, |driver| {
            driver.file_access_progress(self.handle())
        })?;
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

        if progress.total > 0 && progress.completed == progress.total {
            self.pending_transfer
                .take()
                .expect("active transfer retains its file names until completion is observed");
            self.state = DevicePhase::Open;
            Ok(FileTransferStatus::Completed(progress))
        } else {
            Ok(FileTransferStatus::Running(progress))
        }
    }

    /// Polls the active transfer until completion or a local timeout.
    pub fn wait_file_transfer(
        &mut self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<Option<FileProgress>, Error> {
        let started = Instant::now();
        let poll_interval = poll_interval.max(Duration::from_millis(1));
        loop {
            match self.file_transfer_progress()? {
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

    pub fn close(mut self) -> Result<(), DeviceCleanupError> {
        self.cleanup()
    }

    #[cfg(test)]
    pub(crate) fn retained_file_name_addresses_for_test(&self) -> Vec<(usize, usize)> {
        self.pending_transfer
            .iter()
            .map(FileNameBundle::addresses)
            .collect()
    }

    fn cleanup(&mut self) -> Result<(), DeviceCleanupError> {
        drop(self.image_registration.take());
        drop(self.exception_registration.take());
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let stop = if matches!(
            self.state,
            DevicePhase::Measuring
                | DevicePhase::CallbackMeasuring
                | DevicePhase::Faulted(FaultOrigin::AcquisitionStopUncertain)
        ) {
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

        if let Some(names) = self.pending_transfer.take() {
            if close.is_some() {
                // Close 失败时原生异步读取是否结束仍不确定；保留字符串以防悬空指针。
                std::mem::forget(names);
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

    fn require_usable(&self, operation: Operation) -> Result<(), Error> {
        self.require_state(operation, &[DeviceState::Open, DeviceState::Measuring])
    }

    fn require_state(&self, operation: Operation, allowed: &[DeviceState]) -> Result<(), Error> {
        if allowed.contains(&self.state.observable()) {
            Ok(())
        } else {
            Err(Error::InvalidState {
                operation,
                state: self.state.as_str(),
            })
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

fn validated_file_name(operation: Operation, bytes: &[u8]) -> Result<CString, Error> {
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

fn validate_parameter_value(
    operation: Operation,
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

impl Drop for Device {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
