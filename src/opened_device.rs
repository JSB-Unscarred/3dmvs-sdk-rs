use std::sync::Arc;
use std::sync::mpsc::{Receiver, TrySendError, sync_channel};
use std::time::Duration;

use crate::{
    CallbackOptions, DeviceException, DeviceExceptionType, Error, FileProgress, Frame, Image,
    InputViolation, Parameter, ParameterValue, Result, SdkText,
};

/// An opened laser-profiler device with independent session ownership.
///
/// `Device` does not borrow [`crate::Sdk`] and remains usable after that token is dropped. A live
/// device must be closed or dropped before [`crate::Sdk::shutdown`]. `Device` is `Send` but not
/// `Sync`: unique ownership can move to another thread, while calls on different devices may run
/// concurrently. Pull and callback acquisition use one local measurement flag.
pub struct Device {
    inner: mv3d_lp_internal::Device,
}

impl Device {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::Device) -> Self {
        Self { inner }
    }

    /// Starts pull acquisition on this device.
    ///
    /// A failed start leaves the device open so callers may retry.
    pub fn start(&mut self) -> Result<()> {
        self.inner.start()
    }

    /// Waits up to a finite `timeout` for one frame and copies every returned SDK payload.
    ///
    /// A zero duration performs a non-blocking poll. Non-zero sub-millisecond
    /// durations round up to one millisecond. Use [`Self::get_image_blocking`] for the SDK's
    /// infinite wait. A completed wait with no frame is reported as
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
    pub fn get_image(&mut self, timeout: Duration) -> Result<Frame> {
        let timeout_ms = timeout_millis(timeout)?;
        self.inner.get_image(timeout_ms).map(Image::from_internal)
    }

    /// Waits indefinitely for one pull frame using the SDK's infinite-wait sentinel.
    pub fn get_image_blocking(&mut self) -> Result<Frame> {
        self.inner.get_image(u32::MAX).map(Image::from_internal)
    }

    /// Forwards one software trigger; the SDK validates its trigger mode and call order.
    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger()
    }

    /// Stops the active pull or callback acquisition.
    ///
    /// On success, the image callback cookie is retired. A callback that already cloned its sink
    /// may still enqueue one frame after this method returns, so success is not a callback
    /// quiescence barrier. On failure, the acquisition owner stays intact so the caller may retry
    /// or close the device.
    pub fn stop(&mut self) -> Result<()> {
        self.inner.stop()
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
    pub fn start_receiving(&mut self, options: CallbackOptions) -> Result<Receiver<Frame>> {
        let (sink, receiver) = frame_callback_channel(options);
        self.inner.start_callback(sink).map(|()| receiver)
    }

    /// Registers an owned exception-event receiver until it is replaced, disabled, or closed.
    ///
    /// A later call replaces the previous callback after the native registration succeeds. If the
    /// native SDK rejects replacement, its error is returned and the previous Rust registration
    /// remains active.
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
    }

    /// Stops future Rust delivery of exception callbacks.
    ///
    /// The audited native API exposes only registration. This method retires the Rust cookie, so
    /// later native callbacks are ignored safely. A callback that already cloned its sink may
    /// still enqueue one event after this method returns, so this method is not a callback
    /// quiescence barrier. Repeated calls are harmless.
    pub fn disable_exception_delivery(&mut self) {
        self.inner.disable_exception_delivery();
    }

    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer()
    }

    /// Reads one parameter by the SDK's string key.
    pub fn get_parameter(&mut self, key: &str) -> Result<Parameter> {
        self.inner
            .get_parameter(key.as_bytes())
            .map(parameter_from_internal)
    }

    /// Writes one parameter by the SDK's string key.
    pub fn set_parameter(&mut self, key: &str, value: ParameterValue) -> Result<()> {
        let internal_value = parameter_value_to_internal(value);
        self.inner.set_parameter(key.as_bytes(), &internal_value)
    }

    /// Executes one command by the SDK's string key.
    pub fn execute(&mut self, key: &str) -> Result<()> {
        self.inner.execute(key.as_bytes())
    }

    /// Starts copying a file from the device to the host.
    ///
    /// Names are passed as original narrow-string bytes because the vendor SDK does not document
    /// their encoding. Because the asynchronous API does not state whether it copies them, this
    /// device retains both names until another transfer starts successfully or the device closes.
    /// Poll through [`Device::file_transfer_progress`].
    pub fn download_file(&mut self, device_file_name: &[u8], local_file_name: &[u8]) -> Result<()> {
        self.inner.download_file(device_file_name, local_file_name)
    }

    /// Starts copying a host file into the device.
    ///
    /// Names follow the same byte and retained-lifetime contract as [`Device::download_file`].
    pub fn upload_file(&mut self, local_file_name: &[u8], device_file_name: &[u8]) -> Result<()> {
        self.inner.upload_file(local_file_name, device_file_name)
    }

    /// Returns one progress snapshot for the active transfer.
    ///
    /// Values are preserved as the signed `int64_t` fields returned by the SDK; their completion
    /// semantics are intentionally left to the caller because the vendor does not define them.
    pub fn file_transfer_progress(&mut self) -> Result<FileProgress> {
        self.inner.file_transfer_progress()
    }

    /// Stops acquisition when needed and closes the owned handle.
    ///
    /// The consumed owner calls native Close once and reports both Stop and Close failures. Native
    /// Close returning is the callback quiescence barrier even when its status reports an error:
    /// no callback can enqueue afterward, while events already waiting in receivers remain
    /// readable.
    pub fn close(self) -> Result<()> {
        self.inner.close()
    }
}

fn frame_callback_channel(
    options: CallbackOptions,
) -> (mv3d_lp_internal::FrameCallbackSink, Receiver<Frame>) {
    let (sender, receiver) = sync_channel(options.queue_capacity.get());
    let sink =
        Arc::new(move |record| keep_frame_callback(sender.try_send(Image::from_internal(record))));
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
        let description = SdkText::from_sdk_bytes(record.description);
        let event = DeviceException::new(DeviceExceptionType::from_raw(record.kind), description);
        keep_exception_callback(sender.try_send(event))
    });
    (sink, receiver)
}

// 队列满时丢弃最新帧并继续 delivery，避免 SDK callback 线程阻塞。
fn keep_frame_callback<T>(result: std::result::Result<(), TrySendError<T>>) -> bool {
    match result {
        Ok(()) | Err(TrySendError::Full(_)) => true,
        Err(TrySendError::Disconnected(_)) => false,
    }
}

// 队列满或 receiver 断开时终止 delivery，避免继续提供不完整异常流。
fn keep_exception_callback<T>(result: std::result::Result<(), TrySendError<T>>) -> bool {
    result.is_ok()
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

fn parameter_from_internal(record: mv3d_lp_internal::ParameterRecord) -> Parameter {
    match record {
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
            value: SdkText::from_sdk_bytes(value),
            max_length: maximum_length,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::sync::mpsc::TryRecvError;
    use std::time::Duration;

    use crate::{CallbackOptions, Error, InputViolation};

    use super::{exception_callback_channel, frame_callback_channel, timeout_millis};

    // 验证超时转换检查范围并向上取整，防止截断导致实际等待时间缩短。
    #[test]
    fn timeout_conversion_is_finite_checked_and_rounds_up() {
        assert_eq!(timeout_millis(Duration::ZERO).unwrap(), 0);
        assert_eq!(timeout_millis(Duration::from_nanos(1)).unwrap(), 1);
        assert!(matches!(
            timeout_millis(Duration::from_millis(u64::from(u32::MAX))),
            Err(Error::InvalidInput {
                violation: InputViolation::TimeoutTooLong { .. },
                ..
            })
        ));
    }

    // 验证队列满时丢弃最新帧，防止 SDK callback 线程发生阻塞。
    #[test]
    fn production_callback_channel_drops_newest_when_full() {
        let options = CallbackOptions::new(NonZeroUsize::new(1).unwrap());
        let (sink, receiver) = frame_callback_channel(options);

        assert!(sink(callback_frame(1)));
        assert!(sink(callback_frame(2)));
        assert_eq!(receiver.recv().unwrap().frame_number, 1);
        drop(receiver);
        assert!(!sink(callback_frame(3)));
    }

    // 验证异常队列满时终止 delivery，避免静默丢失后继续提供不完整事件流。
    #[test]
    fn exception_callback_channel_stops_delivery_when_full() {
        let options = CallbackOptions::new(NonZeroUsize::new(1).unwrap());
        let (sink, receiver) = exception_callback_channel(options);

        assert!(sink(callback_exception(b"first")));
        assert!(!sink(callback_exception(b"second")));
        assert_eq!(receiver.recv().unwrap().description.as_bytes(), b"first");

        drop(sink);
        assert_eq!(receiver.try_recv(), Err(TryRecvError::Disconnected));
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

    fn callback_exception(description: &[u8]) -> mv3d_lp_internal::ExceptionRecord {
        mv3d_lp_internal::ExceptionRecord {
            kind: 1,
            description: description.to_vec(),
        }
    }
}
