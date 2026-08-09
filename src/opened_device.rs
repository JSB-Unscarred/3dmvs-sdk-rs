use std::sync::Arc;
use std::sync::mpsc::{Receiver, TrySendError, sync_channel};
use std::time::Duration;

use crate::file_transfer::status_from_internal;
use crate::{
    CallbackOptions, CallbackStats, CommandKey, DeviceException, DeviceExceptionType, Error,
    FileTransferStatus, Frame, Image, InputViolation, ParamKey, Parameter, ParameterValue, Result,
    SdkText,
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

/// An opened laser-profiler device with independent session ownership.
///
/// `Device` does not borrow [`crate::Sdk`] and remains usable after that token is dropped. A live
/// device prevents [`crate::Sdk::shutdown`] from finalizing the session. `Device` is `Send` but not
/// `Sync`: unique ownership can move to another thread, while calls on different devices may run
/// concurrently. Pull and callback acquisition are states of this value, so starting and stopping
/// only borrow it briefly.
pub struct Device {
    inner: mv3d_lp_internal::Device,
}

impl Device {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::Device) -> Self {
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
        self.inner
            .get_image(timeout_ms)
            .map(Image::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Waits indefinitely for one pull frame using the SDK's infinite-wait sentinel.
    pub fn get_image_blocking(&mut self) -> Result<Frame> {
        self.inner
            .get_image(u32::MAX)
            .map(Image::from_internal)
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
    pub fn start_receiving(&mut self, options: CallbackOptions) -> Result<Receiver<Frame>> {
        let (sink, receiver) = frame_callback_channel(options);
        self.inner
            .start_callback(sink)
            .map(|()| receiver)
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
    /// their encoding. The SDK marks the descriptor as `[IN]`, so both names are borrowed only for
    /// this start call. Poll through [`Device::file_transfer_progress`] or
    /// [`Device::file_transfer_progress`].
    pub fn download_file(&mut self, device_file_name: &[u8], local_file_name: &[u8]) -> Result<()> {
        self.inner
            .download_file(device_file_name, local_file_name)
            .map_err(Error::map_internal_error)
    }

    /// Starts copying a host file into the device.
    ///
    /// Names follow the same byte and call-scoped borrowing contract as
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

    /// Stops acquisition when needed and closes the owned handle.
    ///
    /// If the first native Close fails, the consumed device's Drop retries once. The returned
    /// error describes the first cleanup attempt.
    pub fn close(self) -> Result<()> {
        self.inner.close().map_err(Error::map_device_cleanup_error)
    }
}

fn frame_callback_channel(
    options: CallbackOptions,
) -> (mv3d_lp_internal::FrameCallbackSink, Receiver<Frame>) {
    let (sender, receiver) = sync_channel(options.queue_capacity.get());
    let sink = Arc::new(move |record| {
        delivery_from_try_send(sender.try_send(Image::from_internal(record)))
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
