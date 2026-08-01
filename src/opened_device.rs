use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, TrySendError, sync_channel};
use std::time::Duration;

use crate::{
    CallbackOptions, CallbackStats, CallbackWorker, CommandKey, DeviceException,
    DeviceExceptionType, Error, FileTransfer, FileTransferStartError, InputViolation, OwnedFrame,
    ParamKey, Parameter, ParameterValue, Result, SdkText,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DeviceState {
    Open,
    Measuring,
    CallbackMeasuring,
    Faulted,
    Transferring,
    CallbackRetired,
}

/// An opened laser-profiler device borrowing its owning [`crate::Sdk`].
///
/// It is intentionally neither `Send` nor `Sync`, and no native handle is
/// exposed. All SDK calls remain serialized inside the private crate.
pub struct Device<'sdk> {
    inner: mv3d_lp_internal::Device<'sdk>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'sdk> Device<'sdk> {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::Device<'sdk>) -> Self {
        Self {
            inner,
            _not_send_or_sync: PhantomData,
        }
    }

    #[must_use]
    pub fn state(&self) -> DeviceState {
        match self.inner.state() {
            mv3d_lp_internal::DeviceState::Open => DeviceState::Open,
            mv3d_lp_internal::DeviceState::Measuring => DeviceState::Measuring,
            mv3d_lp_internal::DeviceState::CallbackMeasuring => DeviceState::CallbackMeasuring,
            mv3d_lp_internal::DeviceState::Faulted => DeviceState::Faulted,
            mv3d_lp_internal::DeviceState::Transferring => DeviceState::Transferring,
            mv3d_lp_internal::DeviceState::CallbackRetired => DeviceState::CallbackRetired,
        }
    }

    /// Starts acquisition and returns an exclusive measurement guard.
    ///
    /// A failed start leaves the device faulted so only cleanup remains valid.
    pub fn start(&mut self) -> Result<Measurement<'_>> {
        self.inner
            .start()
            .map(Measurement::from_internal)
            .map_err(Error::from)
    }

    /// Registers native image delivery, starts measurement, and returns a bounded receiver.
    ///
    /// This is a one-shot mode for the current device handle. After the returned callback
    /// measurement stops, close and reopen the device before starting another acquisition.
    ///
    /// # Native contract
    ///
    /// For the audited LPSDK 1.3.3.3 runtime, this wrapper assumes that each callback descriptor
    /// and payload remains readable and unchanged until the native callback returns. The wrapper
    /// copies every payload before returning from that callback, but the vendor does not provide
    /// a separate written guarantee for this stability window.
    pub fn start_receiving(
        &mut self,
        options: CallbackOptions,
    ) -> Result<(CallbackMeasurement<'_>, Receiver<OwnedFrame>)> {
        let (sink, receiver) = frame_callback_channel(options)?;
        self.inner
            .start_callback(sink)
            .map(|inner| (CallbackMeasurement::from_internal(inner), receiver))
            .map_err(Error::from)
    }

    /// Starts callback acquisition and invokes `handler` serially on a Rust worker thread.
    ///
    /// Like [`Device::start_receiving`], this is a one-shot image registration for the current
    /// device handle.
    pub fn start_with_callback<F>(
        &mut self,
        options: CallbackOptions,
        handler: F,
    ) -> Result<(CallbackMeasurement<'_>, CallbackWorker)>
    where
        F: FnMut(OwnedFrame) + Send + 'static,
    {
        let (sink, receiver) = frame_callback_channel(options)?;
        let worker =
            CallbackWorker::spawn(receiver, handler).map_err(|_| Error::CallbackWorkerSpawn)?;
        self.inner
            .start_callback(sink)
            .map(|inner| (CallbackMeasurement::from_internal(inner), worker))
            .map_err(Error::from)
    }

    /// Registers an owned exception-event receiver for the lifetime of this device handle.
    ///
    /// Exception registration is one-shot. This method and [`Device::on_exception`] are mutually
    /// exclusive for a device handle, even if the receiver or worker is later dropped.
    /// The audited LPSDK 1.3.3.3 contract assumes that each exception descriptor remains readable
    /// until the native callback returns; the event is copied within that window, which is not a
    /// separate written vendor guarantee.
    pub fn exception_receiver(
        &mut self,
        options: CallbackOptions,
    ) -> Result<Receiver<DeviceException>> {
        let (sink, receiver) = exception_callback_channel(options)?;
        self.inner
            .register_exception_callback(sink)
            .map(|()| receiver)
            .map_err(Error::from)
    }

    /// Invokes an exception handler serially on a Rust worker thread.
    ///
    /// This consumes the same one-shot exception registration as
    /// [`Device::exception_receiver`].
    pub fn on_exception<F>(
        &mut self,
        options: CallbackOptions,
        handler: F,
    ) -> Result<CallbackWorker>
    where
        F: FnMut(DeviceException) + Send + 'static,
    {
        let (sink, receiver) = exception_callback_channel(options)?;
        let worker =
            CallbackWorker::spawn(receiver, handler).map_err(|_| Error::CallbackWorkerSpawn)?;
        self.inner
            .register_exception_callback(sink)
            .map(|()| worker)
            .map_err(Error::from)
    }

    #[must_use]
    pub fn exception_callback_stats(&self) -> Option<CallbackStats> {
        self.inner
            .exception_callback_stats()
            .map(CallbackStats::from_internal)
    }

    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer().map_err(Error::from)
    }

    pub fn get_parameter(&mut self, key: &ParamKey) -> Result<Parameter> {
        self.inner
            .get_parameter(key.as_bytes())
            .map_err(Error::from)
            .and_then(parameter_from_internal)
    }

    pub fn set_parameter(&mut self, key: &ParamKey, value: ParameterValue) -> Result<()> {
        let internal_value = parameter_value_to_internal(value);
        self.inner
            .set_parameter(key.as_bytes(), &internal_value)
            .map_err(Error::from)
    }

    pub fn execute(&mut self, key: &CommandKey) -> Result<()> {
        self.inner.execute(key.as_bytes()).map_err(Error::from)
    }

    /// Starts copying a file from the device to the host.
    ///
    /// This consumes the device because the active transfer owns it. Names are passed as original
    /// narrow-string bytes because the vendor SDK does not document their encoding. The wrapper
    /// retains every transfer's names until a successful device close. After completion, use
    /// [`FileTransfer::try_into_device`] to recover the device.
    pub fn download_file(
        self,
        device_file_name: &[u8],
        local_file_name: &[u8],
    ) -> std::result::Result<FileTransfer<'sdk>, FileTransferStartError<'sdk>> {
        self.inner
            .download_file(device_file_name, local_file_name)
            .map(FileTransfer::from_internal)
            .map_err(FileTransferStartError::from_internal)
    }

    /// Starts copying a host file into the device.
    ///
    /// This uses the same retained-name and native termination assumptions as
    /// [`Device::download_file`].
    pub fn upload_file(
        self,
        local_file_name: &[u8],
        device_file_name: &[u8],
    ) -> std::result::Result<FileTransfer<'sdk>, FileTransferStartError<'sdk>> {
        self.inner
            .upload_file(local_file_name, device_file_name)
            .map(FileTransfer::from_internal)
            .map_err(FileTransferStartError::from_internal)
    }

    pub fn close(self) -> Result<()> {
        self.inner.close().map_err(Error::from)
    }
}

/// An active native-callback acquisition session borrowing its device exclusively.
///
/// The guard revokes and drains Rust callback dispatch before stopping the SDK. It intentionally
/// exposes neither pull acquisition nor `clear_buffer`.
#[must_use = "dropping CallbackMeasurement stops callback acquisition"]
pub struct CallbackMeasurement<'device> {
    inner: mv3d_lp_internal::CallbackMeasurement<'device>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'device> CallbackMeasurement<'device> {
    fn from_internal(inner: mv3d_lp_internal::CallbackMeasurement<'device>) -> Self {
        Self {
            inner,
            _not_send_or_sync: PhantomData,
        }
    }

    #[must_use]
    pub fn state(&self) -> DeviceState {
        match self.inner.state() {
            mv3d_lp_internal::DeviceState::Open => DeviceState::Open,
            mv3d_lp_internal::DeviceState::Measuring => DeviceState::Measuring,
            mv3d_lp_internal::DeviceState::CallbackMeasuring => DeviceState::CallbackMeasuring,
            mv3d_lp_internal::DeviceState::Faulted => DeviceState::Faulted,
            mv3d_lp_internal::DeviceState::Transferring => DeviceState::Transferring,
            mv3d_lp_internal::DeviceState::CallbackRetired => DeviceState::CallbackRetired,
        }
    }

    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger().map_err(Error::from)
    }

    #[must_use]
    pub fn callback_stats(&self) -> CallbackStats {
        CallbackStats::from_internal(self.inner.callback_stats())
    }

    pub fn stop(self) -> Result<()> {
        self.inner.stop().map_err(Error::from)
    }
}

fn frame_callback_channel(
    options: CallbackOptions,
) -> Result<(mv3d_lp_internal::FrameCallbackSink, Receiver<OwnedFrame>)> {
    let capacity = callback_queue_capacity(options)?;
    let (sender, receiver) = sync_channel(capacity);
    let sink = Arc::new(move |record| {
        delivery_from_try_send(sender.try_send(OwnedFrame::from_internal(record)))
    });
    Ok((sink, receiver))
}

fn exception_callback_channel(
    options: CallbackOptions,
) -> Result<(
    mv3d_lp_internal::ExceptionCallbackSink,
    Receiver<DeviceException>,
)> {
    let capacity = callback_queue_capacity(options)?;
    let (sender, receiver) = sync_channel(capacity);
    let sink = Arc::new(move |record: mv3d_lp_internal::ExceptionRecord| {
        let Ok(description) = SdkText::try_from(record.description) else {
            return mv3d_lp_internal::CallbackDelivery::Disconnected;
        };
        let event = DeviceException::new(DeviceExceptionType::from_raw(record.kind), description);
        delivery_from_try_send(sender.try_send(event))
    });
    Ok((sink, receiver))
}

fn callback_queue_capacity(options: CallbackOptions) -> Result<usize> {
    let capacity = options.queue_capacity.get();
    if capacity > CallbackOptions::MAX_QUEUE_CAPACITY {
        return Err(Error::InvalidInput {
            field: "callback queue capacity",
            violation: InputViolation::CallbackQueueCapacity {
                maximum: CallbackOptions::MAX_QUEUE_CAPACITY,
                actual: capacity,
            },
        });
    }
    Ok(capacity)
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

/// An active pull-acquisition session borrowing its device exclusively.
///
/// Dropping this guard makes a best-effort attempt to stop measurement. Call
/// [`Measurement::stop`] to observe a stop error explicitly.
#[must_use = "dropping Measurement stops acquisition"]
pub struct Measurement<'device> {
    inner: mv3d_lp_internal::Measurement<'device>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'device> Measurement<'device> {
    fn from_internal(inner: mv3d_lp_internal::Measurement<'device>) -> Self {
        Self {
            inner,
            _not_send_or_sync: PhantomData,
        }
    }

    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger().map_err(Error::from)
    }

    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer().map_err(Error::from)
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
    /// copy. The process-wide lock prevents intervening wrapper calls, but cannot control private
    /// SDK worker threads; the vendor does not provide a separate written guarantee for this
    /// stability window.
    pub fn get_image(&mut self, timeout: Duration) -> Result<OwnedFrame> {
        let timeout_ms = timeout_millis(timeout)?;
        self.inner
            .get_image(timeout_ms)
            .map(OwnedFrame::from_internal)
            .map_err(Error::from)
    }

    pub fn get_parameter(&mut self, key: &ParamKey) -> Result<Parameter> {
        self.inner
            .get_parameter(key.as_bytes())
            .map_err(Error::from)
            .and_then(parameter_from_internal)
    }

    pub fn set_parameter(&mut self, key: &ParamKey, value: ParameterValue) -> Result<()> {
        let internal_value = parameter_value_to_internal(value);
        self.inner
            .set_parameter(key.as_bytes(), &internal_value)
            .map_err(Error::from)
    }

    pub fn execute(&mut self, key: &CommandKey) -> Result<()> {
        self.inner.execute(key.as_bytes()).map_err(Error::from)
    }

    pub fn stop(self) -> Result<()> {
        self.inner.stop().map_err(Error::from)
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
            value: SdkText::new(value)?,
            max_length: maximum_length,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::time::Duration;

    use crate::{CallbackOptions, Error, InputViolation};

    use super::{callback_queue_capacity, frame_callback_channel, timeout_millis};

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

    #[test]
    fn excessive_callback_queue_capacity_is_rejected_before_allocation() {
        let options = CallbackOptions::new(
            NonZeroUsize::new(CallbackOptions::MAX_QUEUE_CAPACITY + 1).unwrap(),
        );
        assert!(matches!(
            callback_queue_capacity(options),
            Err(Error::InvalidInput {
                field: "callback queue capacity",
                violation: InputViolation::CallbackQueueCapacity { .. },
            })
        ));
    }

    #[test]
    fn production_callback_channel_drops_newest_when_full() {
        let options = CallbackOptions::new(NonZeroUsize::new(1).unwrap());
        let (sink, receiver) = frame_callback_channel(options).unwrap();

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
