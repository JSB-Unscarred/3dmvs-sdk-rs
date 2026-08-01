use std::net::Ipv4Addr;
use std::num::NonZeroUsize;
use std::sync::mpsc::Receiver;

use mv3d_lp::{
    CallbackMeasurement, CallbackOptions, CallbackStats, CallbackWorker, Device, DeviceException,
    DeviceExceptionType, DeviceInfo, DeviceState, Error, FileProgress, FileTransfer,
    FileTransferStartError, ImageFileFormat, ImageProcessor, ImageType, IpConfiguration,
    Measurement, OwnedFrame, OwnedImage, Result, Sdk, SdkText,
};

#[test]
#[allow(clippy::type_complexity)]
fn public_lifecycle_and_control_methods_have_safe_signatures() {
    let _: fn() -> Result<Sdk> = Sdk::initialize;
    let _: for<'sdk> fn(&'sdk Sdk, Ipv4Addr) -> Result<Device<'sdk>> = Sdk::open_by_ip;
    let _: for<'sdk> fn(&'sdk Sdk, &mv3d_lp::SerialNumber) -> Result<Device<'sdk>> =
        Sdk::open_by_serial;
    let _: fn(Sdk) -> Result<()> = Sdk::shutdown;
    let _: for<'sdk> fn(&'sdk Sdk) -> ImageProcessor<'sdk> = Sdk::image_processor;
    let _ = Device::start;
    let _: for<'device> fn(
        &'device mut Device<'static>,
        CallbackOptions,
    ) -> Result<(CallbackMeasurement<'device>, Receiver<OwnedFrame>)> =
        Device::<'static>::start_receiving;
    let _: for<'device> fn(
        &'device mut Device<'static>,
        CallbackOptions,
        fn(OwnedFrame),
    ) -> Result<(CallbackMeasurement<'device>, CallbackWorker)> =
        Device::<'static>::start_with_callback::<fn(OwnedFrame)>;
    let _: for<'device> fn(
        &'device mut Device<'static>,
        CallbackOptions,
    ) -> Result<Receiver<DeviceException>> = Device::<'static>::exception_receiver;
    let _: for<'device> fn(
        &'device mut Device<'static>,
        CallbackOptions,
        fn(DeviceException),
    ) -> Result<CallbackWorker> = Device::<'static>::on_exception::<fn(DeviceException)>;
    let _ = Device::clear_buffer;
    let _: fn(&CallbackMeasurement<'static>) -> DeviceState = CallbackMeasurement::<'static>::state;
    let _: fn(&mut CallbackMeasurement<'static>) -> Result<()> =
        CallbackMeasurement::<'static>::soft_trigger;
    let _: fn(&CallbackMeasurement<'static>) -> CallbackStats =
        CallbackMeasurement::<'static>::callback_stats;
    let _: fn(CallbackMeasurement<'static>) -> Result<()> = CallbackMeasurement::<'static>::stop;
    let _ = Measurement::get_image;
    let _ = Measurement::soft_trigger;
    let _ = Measurement::clear_buffer;
    let _ = Measurement::stop;
    let _: fn(
        Device<'static>,
        &[u8],
        &[u8],
    ) -> std::result::Result<FileTransfer<'static>, FileTransferStartError<'static>> =
        Device::<'static>::download_file;
    let _: fn(
        Device<'static>,
        &[u8],
        &[u8],
    ) -> std::result::Result<FileTransfer<'static>, FileTransferStartError<'static>> =
        Device::<'static>::upload_file;
    let _: fn(
        FileTransfer<'static>,
    ) -> std::result::Result<(Device<'static>, FileProgress), FileTransfer<'static>> =
        FileTransfer::<'static>::try_into_device;
    let _: fn(FileTransfer<'static>) -> Result<()> = FileTransfer::<'static>::close;
    let _: fn(
        FileTransferStartError<'static>,
    )
        -> std::result::Result<(Error, Device<'static>), FileTransferStartError<'static>> =
        FileTransferStartError::<'static>::into_rejected_device;
    let _ = ImageProcessor::depth_to_point_cloud;
    let _ = ImageProcessor::depth_to_round_point_cloud;
    let _ = ImageProcessor::convert;
    let _ = ImageProcessor::mosaic_depth;
    let _ = ImageProcessor::save;
    let _: ImageFileFormat = ImageFileFormat::Ply;
}

#[test]
fn owned_output_types_can_move_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<DeviceInfo>();
    assert_send_sync::<SdkText>();
    assert_send_sync::<IpConfiguration>();
    assert_send_sync::<OwnedFrame>();
    assert_send_sync::<OwnedImage>();
    assert_send_sync::<DeviceException>();
    assert_send_sync::<DeviceExceptionType>();
    assert_send_sync::<CallbackOptions>();
    assert_send_sync::<CallbackStats>();
}

#[test]
fn callback_public_types_preserve_owned_data_and_counters() {
    fn assert_exception_fields(event: &DeviceException) {
        let _: DeviceExceptionType = event.kind;
        let _: &SdkText = &event.description;
    }

    fn assert_stats_fields(stats: &CallbackStats) {
        let _: u64 = stats.delivered;
        let _: u64 = stats.dropped_full;
        let _: u64 = stats.invalid_payloads;
        let _: u64 = stats.panics;
        let _: bool = stats.accepting;
    }

    let _: fn(&DeviceException) = assert_exception_fields;
    let _: fn(&CallbackStats) = assert_stats_fields;
    let _: NonZeroUsize = CallbackOptions::default().queue_capacity;

    assert_eq!(DeviceExceptionType::DISCONNECTED.bits(), 1);
    assert_eq!(DeviceExceptionType::UNDEFINED.raw(), -1);
    let unknown = DeviceExceptionType::from_bits(0xDEAD_BEEF);
    assert_eq!(unknown.bits(), 0xDEAD_BEEF);
    assert_eq!(unknown.name(), None);
}

#[test]
fn image_types_preserve_known_and_unknown_bits() {
    assert_eq!(ImageType::MONO8.bits(), 0x0108_0001);
    assert_eq!(ImageType::UNDEFINED.raw(), -1);

    let unknown = ImageType::from_bits(0xDEAD_BEEF);
    assert_eq!(unknown.bits(), 0xDEAD_BEEF);
    assert_eq!(unknown.raw(), 0xDEAD_BEEF_u32 as i32);
    assert_eq!(unknown.name(), None);
    assert!(format!("{unknown:?}").contains("0xDEADBEEF"));
}

#[test]
fn owned_frame_exposes_owned_payload_and_metadata() {
    fn assert_fields(frame: &OwnedFrame) {
        let _: ImageType = frame.image_type;
        let _: u32 = frame.width;
        let _: u32 = frame.height;
        let _: &Vec<u8> = &frame.data;
        let _: &Option<Vec<u8>> = &frame.intensity_data;
        let _: &Option<Vec<i64>> = &frame.exposure_timestamps;
        let _: u32 = frame.frame_number;
        let _: i64 = frame.device_timestamp;
        let _: bool = frame.valid;
        let _: f32 = frame.x_scale;
        let _: f32 = frame.y_scale;
        let _: f32 = frame.z_scale;
        let _: i32 = frame.x_offset;
        let _: i32 = frame.y_offset;
        let _: i32 = frame.z_offset;
    }

    let _: fn(&OwnedFrame) = assert_fields;
}
