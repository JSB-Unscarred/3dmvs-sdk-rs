use std::sync::Arc;
use std::sync::mpsc::{Receiver, TrySendError, sync_channel};
use std::time::Duration;

use crate::file_transfer::{progress_from_internal, status_from_internal};
use crate::{
    CallbackOptions, CallbackStats, CallbackWorker, CommandKey, DeviceException,
    DeviceExceptionType, Error, FileProgress, FileTransferStatus, InputViolation, OwnedFrame,
    ParamKey, Parameter, ParameterValue, Result, SdkText,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum DeviceState {
    Open,
    Measuring,
    CallbackMeasuring,
    Faulted,
    Transferring,
}

/// An opened laser-profiler device borrowing its owning [`crate::Sdk`].
///
/// `Device` is `Send` but not `Sync`: unique ownership may be handed to another thread, but the
/// device cannot be shared for concurrent calls. It continues to borrow its owning [`crate::Sdk`],
/// so direct handoff normally uses [`std::thread::scope`]. For a long-lived owner thread, create
/// both the SDK and device inside that thread. Pull and callback acquisition are states of this
/// value, so starting or stopping only borrows it briefly. No native handle is exposed; unique
/// ownership serializes calls on this device while calls on different devices may run concurrently.
pub struct Device<'sdk> {
    inner: mv3d_lp_internal::Device<'sdk>,
}

impl<'sdk> Device<'sdk> {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::Device<'sdk>) -> Self {
        Self { inner }
    }

    #[must_use]
    pub fn state(&self) -> DeviceState {
        state_from_internal(self.inner.state())
    }

    /// Starts pull acquisition on this device.
    ///
    /// A failed start leaves the device open so callers may retry.
    pub fn start(&mut self) -> Result<()> {
        self.inner.start().map_err(Error::map_internal_error)
    }

    /// Waits up to `timeout` for one frame and copies every returned SDK payload.
    ///
    /// A zero duration performs a non-blocking poll. Non-zero sub-millisecond
    /// durations round up to one millisecond, and the SDK's infinite-wait
    /// sentinel is rejected. A completed wait with no frame is reported as
    /// [`crate::StatusCode::NO_DATA`] through [`crate::Error::Sdk`].
    ///
    /// # Native contract
    ///
    /// For the audited LPSDK 1.3.3.3 runtime, this wrapper assumes that a successful call's
    /// descriptor and payloads remain readable and unchanged during the immediate synchronous
    /// copy. Unique `Device` ownership prevents another safe call on the same handle, and official
    /// multi-camera examples support distinct handles running concurrently. The wrapper cannot
    /// control private SDK worker threads; the vendor does not separately document this stability
    /// window.
    pub fn get_image(&mut self, timeout: Duration) -> Result<OwnedFrame> {
        let timeout_ms = timeout_millis(timeout)?;
        self.inner
            .get_image(timeout_ms)
            .map(OwnedFrame::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Sends one software trigger while pull or callback acquisition is active.
    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger().map_err(Error::map_internal_error)
    }

    /// Stops the active pull or callback acquisition.
    ///
    /// Callback dispatch is revoked and drained before the SDK is stopped. A failed stop faults
    /// the device so normal operations cannot continue with an uncertain native state.
    pub fn stop(&mut self) -> Result<()> {
        self.inner.stop().map_err(Error::map_internal_error)
    }

    /// Registers native image delivery, starts measurement, and returns a bounded receiver.
    ///
    /// After [`Device::stop`] succeeds, another callback acquisition may be started on the same
    /// device handle. If the native SDK rejects re-registration, its error is returned to the
    /// caller.
    ///
    /// # Native contract
    ///
    /// For the audited LPSDK 1.3.3.3 runtime, this wrapper assumes that each callback descriptor
    /// and payload remains readable and unchanged until the native callback returns. The wrapper
    /// copies every payload before returning from that callback, but the vendor does not provide
    /// a separate written guarantee for this stability window.
    pub fn start_receiving(&mut self, options: CallbackOptions) -> Result<Receiver<OwnedFrame>> {
        let (sink, receiver) = frame_callback_channel(options);
        self.inner
            .start_callback(sink)
            .map(|()| receiver)
            .map_err(Error::map_internal_error)
    }

    /// Starts callback acquisition and invokes `handler` serially on a Rust worker thread.
    ///
    /// Like [`Device::start_receiving`], this can be called again after the previous callback
    /// acquisition stops successfully. Call [`Device::stop`] before [`CallbackWorker::join`],
    /// because the active registration owns the worker's channel sender.
    pub fn start_with_callback<F>(
        &mut self,
        options: CallbackOptions,
        handler: F,
    ) -> Result<CallbackWorker>
    where
        F: FnMut(OwnedFrame) + Send + 'static,
    {
        let (sink, receiver) = frame_callback_channel(options);
        let worker =
            CallbackWorker::spawn(receiver, handler).map_err(|_| Error::CallbackWorkerSpawn)?;
        self.inner
            .start_callback(sink)
            .map(|()| worker)
            .map_err(Error::map_internal_error)
    }

    /// Returns image callback counters while callback acquisition is active.
    ///
    /// Stopping callback acquisition retires its registration, after which this returns `None`.
    #[must_use]
    pub fn image_callback_stats(&self) -> Option<CallbackStats> {
        self.inner
            .image_callback_stats()
            .map(CallbackStats::from_internal)
    }

    /// Registers an owned exception-event receiver until it is replaced, disabled, or closed.
    ///
    /// A later call to this method or [`Device::on_exception`] replaces the previous callback
    /// after the native registration succeeds. If the native SDK rejects replacement, its error
    /// is returned and the previous Rust registration remains active.
    /// The audited LPSDK 1.3.3.3 contract assumes that each exception descriptor remains readable
    /// until the native callback returns; the event is copied within that window, which is not a
    /// separate written vendor guarantee.
    pub fn exception_receiver(
        &mut self,
        options: CallbackOptions,
    ) -> Result<Receiver<DeviceException>> {
        let (sink, receiver) = exception_callback_channel(options);
        self.inner
            .register_exception_callback(sink)
            .map(|()| receiver)
            .map_err(Error::map_internal_error)
    }

    /// Invokes an exception handler serially on a Rust worker thread.
    ///
    /// A later call to this method or [`Device::exception_receiver`] replaces the previous
    /// exception callback after the native registration succeeds. Call
    /// [`Device::disable_exception_delivery`] before joining the returned worker so its channel
    /// closes.
    pub fn on_exception<F>(
        &mut self,
        options: CallbackOptions,
        handler: F,
    ) -> Result<CallbackWorker>
    where
        F: FnMut(DeviceException) + Send + 'static,
    {
        let (sink, receiver) = exception_callback_channel(options);
        let worker =
            CallbackWorker::spawn(receiver, handler).map_err(|_| Error::CallbackWorkerSpawn)?;
        self.inner
            .register_exception_callback(sink)
            .map(|()| worker)
            .map_err(Error::map_internal_error)
    }

    #[must_use]
    pub fn exception_callback_stats(&self) -> Option<CallbackStats> {
        self.inner
            .exception_callback_stats()
            .map(CallbackStats::from_internal)
    }

    /// Stops Rust delivery of exception callbacks and drains callbacks already in flight.
    ///
    /// The audited native API exposes only registration. This method retires the Rust cookie, so
    /// later native callbacks are ignored safely. Repeated calls are harmless.
    pub fn disable_exception_delivery(&mut self) {
        self.inner.disable_exception_delivery();
    }

    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer().map_err(Error::map_internal_error)
    }

    pub fn get_parameter(&mut self, key: &ParamKey) -> Result<Parameter> {
        self.inner
            .get_parameter(key.as_bytes())
            .map_err(Error::map_internal_error)
            .and_then(parameter_from_internal)
    }

    pub fn set_parameter(&mut self, key: &ParamKey, value: ParameterValue) -> Result<()> {
        let internal_value = parameter_value_to_internal(value);
        self.inner
            .set_parameter(key.as_bytes(), &internal_value)
            .map_err(Error::map_internal_error)
    }

    pub fn execute(&mut self, key: &CommandKey) -> Result<()> {
        self.inner
            .execute(key.as_bytes())
            .map_err(Error::map_internal_error)
    }

    /// Starts copying a file from the device to the host.
    ///
    /// Names are passed as original narrow-string bytes because the vendor SDK does not document
    /// their encoding. The wrapper retains the active transfer's names until completion or a
    /// successful device close. If close fails, it intentionally leaks those names because native
    /// termination is uncertain. Poll through [`Device::file_transfer_progress`] or
    /// [`Device::wait_file_transfer`].
    pub fn download_file(&mut self, device_file_name: &[u8], local_file_name: &[u8]) -> Result<()> {
        self.inner
            .download_file(device_file_name, local_file_name)
            .map_err(Error::map_internal_error)
    }

    /// Starts copying a host file into the device.
    ///
    /// This uses the same retained-name and native termination assumptions as
    /// [`Device::download_file`].
    pub fn upload_file(&mut self, local_file_name: &[u8], device_file_name: &[u8]) -> Result<()> {
        self.inner
            .upload_file(local_file_name, device_file_name)
            .map_err(Error::map_internal_error)
    }

    /// Returns one progress snapshot for the active transfer.
    ///
    /// Completion returns the device to [`DeviceState::Open`]. A polling error ends only this
    /// call, so the transfer remains available for another poll.
    pub fn file_transfer_progress(&mut self) -> Result<FileTransferStatus> {
        self.inner
            .file_transfer_progress()
            .map(status_from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Polls the active transfer until completion or until `timeout` elapses.
    ///
    /// `Ok(None)` means the timeout elapsed while the transfer was still running. A polling error
    /// ends only this call, so callers may retry. Each successful progress snapshot is validated
    /// independently because the SDK does not promise monotonic counters.
    pub fn wait_file_transfer(
        &mut self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<Option<FileProgress>> {
        self.inner
            .wait_file_transfer(poll_interval, timeout)
            .map(|progress| progress.map(progress_from_internal))
            .map_err(Error::map_internal_error)
    }

    pub fn close(self) -> Result<()> {
        self.inner.close().map_err(Error::map_device_cleanup_error)
    }
}

fn frame_callback_channel(
    options: CallbackOptions,
) -> (mv3d_lp_internal::FrameCallbackSink, Receiver<OwnedFrame>) {
    let (sender, receiver) = sync_channel(options.queue_capacity.get());
    let sink = Arc::new(move |record| {
        delivery_from_try_send(sender.try_send(OwnedFrame::from_internal(record)))
    });
    (sink, receiver)
}

fn exception_callback_channel(
    options: CallbackOptions,
) -> (
    mv3d_lp_internal::ExceptionCallbackSink,
    Receiver<DeviceException>,
) {
    let (sender, receiver) = sync_channel(options.queue_capacity.get());
    let sink = Arc::new(move |record: mv3d_lp_internal::ExceptionRecord| {
        let Ok(description) = SdkText::try_from(record.description) else {
            return mv3d_lp_internal::CallbackDelivery::Disconnected;
        };
        let event = DeviceException::new(DeviceExceptionType::from_raw(record.kind), description);
        delivery_from_try_send(sender.try_send(event))
    });
    (sink, receiver)
}

fn delivery_from_try_send<T>(
    result: std::result::Result<(), TrySendError<T>>,
) -> mv3d_lp_internal::CallbackDelivery {
    match result {
        Ok(()) => mv3d_lp_internal::CallbackDelivery::Delivered,
        Err(TrySendError::Full(_)) => mv3d_lp_internal::CallbackDelivery::Full,
        Err(TrySendError::Disconnected(_)) => mv3d_lp_internal::CallbackDelivery::Disconnected,
    }
}

fn state_from_internal(state: mv3d_lp_internal::DeviceState) -> DeviceState {
    match state {
        mv3d_lp_internal::DeviceState::Open => DeviceState::Open,
        mv3d_lp_internal::DeviceState::Measuring => DeviceState::Measuring,
        mv3d_lp_internal::DeviceState::CallbackMeasuring => DeviceState::CallbackMeasuring,
        mv3d_lp_internal::DeviceState::Faulted => DeviceState::Faulted,
        mv3d_lp_internal::DeviceState::Transferring => DeviceState::Transferring,
    }
}

fn timeout_millis(timeout: Duration) -> Result<u32> {
    const NANOS_PER_MILLI: u128 = 1_000_000;
    const MAXIMUM_MILLIS: u32 = u32::MAX - 1;

    let millis = timeout.as_nanos().div_ceil(NANOS_PER_MILLI);
    if millis > u128::from(MAXIMUM_MILLIS) {
        return Err(Error::InvalidInput {
            field: "timeout",
            violation: InputViolation::TimeoutTooLong {
                maximum_millis: MAXIMUM_MILLIS,
                actual_millis: millis,
            },
        });
    }
    Ok(millis as u32)
}

fn parameter_value_to_internal(value: ParameterValue) -> mv3d_lp_internal::ParameterValueRecord {
    match value {
        ParameterValue::Bool(value) => mv3d_lp_internal::ParameterValueRecord::Bool(value),
        ParameterValue::Integer(value) => mv3d_lp_internal::ParameterValueRecord::Integer(value),
        ParameterValue::Float(value) => mv3d_lp_internal::ParameterValueRecord::Float(value),
        ParameterValue::Enumeration(value) => {
            mv3d_lp_internal::ParameterValueRecord::Enumeration(value)
        }
        ParameterValue::String(value) => {
            mv3d_lp_internal::ParameterValueRecord::String(value.into_bytes())
        }
    }
}

fn parameter_from_internal(record: mv3d_lp_internal::ParameterRecord) -> Result<Parameter> {
    Ok(match record {
        mv3d_lp_internal::ParameterRecord::Bool(value) => Parameter::Bool(value),
        mv3d_lp_internal::ParameterRecord::Integer {
            value,
            minimum,
            maximum,
            increment,
        } => Parameter::Integer {
            value,
            min: minimum,
            max: maximum,
            increment,
        },
        mv3d_lp_internal::ParameterRecord::Float {
            value,
            minimum,
            maximum,
        } => Parameter::Float {
            value,
            min: minimum,
            max: maximum,
        },
        mv3d_lp_internal::ParameterRecord::Enumeration { value, supported } => {
            Parameter::Enumeration { value, supported }
        }
        mv3d_lp_internal::ParameterRecord::String {
            value,
            maximum_length,
        } => Parameter::String {
            value: SdkText::try_from(value)?,
            max_length: maximum_length,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use crate::{CallbackOptions, Error, InputViolation};

    use super::{frame_callback_channel, timeout_millis};

    // 验证超时转换检查范围并向上取整，防止截断导致实际等待时间缩短。
    #[test]
    fn timeout_conversion_is_finite_checked_and_rounds_up() {
        assert_eq!(timeout_millis(Duration::ZERO).unwrap(), 0);
        assert_eq!(timeout_millis(Duration::from_nanos(1)).unwrap(), 1);
        assert_eq!(timeout_millis(Duration::from_millis(37)).unwrap(), 37);
        assert_eq!(
            timeout_millis(Duration::from_millis(u64::from(u32::MAX - 1))).unwrap(),
            u32::MAX - 1
        );
        assert!(matches!(
            timeout_millis(
                Duration::from_millis(u64::from(u32::MAX - 1)) + Duration::from_nanos(1)
            ),
            Err(Error::InvalidInput {
                violation: InputViolation::TimeoutTooLong { .. },
                ..
            })
        ));
        assert!(matches!(
            timeout_millis(Duration::from_millis(u64::from(u32::MAX))),
            Err(Error::InvalidInput {
                violation: InputViolation::TimeoutTooLong { .. },
                ..
            })
        ));
    }

    // 验证生产 callback channel 使用配置容量，防止公开选项与实际队列脱节。
    #[test]
    fn production_callback_channel_honors_configured_capacity() {
        const CAPACITY: usize = 65;

        let options = CallbackOptions::new(NonZeroUsize::new(CAPACITY).unwrap());
        let (sink, receiver) = frame_callback_channel(options);

        for frame_number in 0..CAPACITY {
            assert_eq!(
                sink(callback_frame(u32::try_from(frame_number).unwrap())),
                mv3d_lp_internal::CallbackDelivery::Delivered
            );
        }
        assert_eq!(
            sink(callback_frame(u32::try_from(CAPACITY).unwrap())),
            mv3d_lp_internal::CallbackDelivery::Full
        );

        for frame_number in 0..CAPACITY {
            assert_eq!(
                receiver.recv().unwrap().frame_number,
                u32::try_from(frame_number).unwrap()
            );
        }
    }

    // 验证队列满时丢弃最新帧，防止 SDK callback 线程发生阻塞。
    #[test]
    fn production_callback_channel_drops_newest_when_full() {
        let options = CallbackOptions::new(NonZeroUsize::new(1).unwrap());
        let (sink, receiver) = frame_callback_channel(options);

        assert_eq!(
            sink(callback_frame(1)),
            mv3d_lp_internal::CallbackDelivery::Delivered
        );
        assert_eq!(
            sink(callback_frame(2)),
            mv3d_lp_internal::CallbackDelivery::Full
        );
        assert_eq!(receiver.recv().unwrap().frame_number, 1);
        drop(receiver);
        assert_eq!(
            sink(callback_frame(3)),
            mv3d_lp_internal::CallbackDelivery::Disconnected
        );
    }

    fn callback_frame(frame_number: u32) -> mv3d_lp_internal::FrameRecord {
        mv3d_lp_internal::FrameRecord {
            image_type: mv3d_lp_internal::ImageTypeRecord::from_bits(0x0108_0001),
            width: 1,
            height: 1,
            data: vec![frame_number as u8],
            intensity_data: None,
            exposure_timestamps: None,
            frame_number,
            device_timestamp: 0,
            valid: true,
            x_scale: 0.0,
            y_scale: 0.0,
            z_scale: 0.0,
            x_offset: 0,
            y_offset: 0,
            z_offset: 0,
        }
    }
}
