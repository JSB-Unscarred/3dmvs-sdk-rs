use std::net::Ipv4Addr;
use std::num::NonZeroUsize;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use mv3d_lp::{
    CallbackOptions, CallbackStats, CallbackWorker, Device, DeviceException, DeviceExceptionType,
    DeviceState, FileProgress, FileTransferStatus, ImageFileFormat, ImageProcessor, ImageType,
    OwnedFrame, Result, Sdk, SdkText,
};

// 验证公开生命周期与控制函数保持安全签名，防止借用关系或所有权约定退化。
#[test]
#[allow(clippy::type_complexity)]
fn public_lifecycle_and_control_methods_have_safe_signatures() {
    let _: fn() -> Result<Sdk> = Sdk::initialize;
    let _: for<'sdk> fn(&'sdk Sdk, Ipv4Addr) -> Result<Device<'sdk>> = Sdk::open_by_ip;
    let _: for<'sdk> fn(&'sdk Sdk, &mv3d_lp::SerialNumber) -> Result<Device<'sdk>> =
        Sdk::open_by_serial;
    let _: fn(Sdk) -> Result<()> = Sdk::shutdown;
    let _: for<'sdk> fn(&'sdk Sdk) -> ImageProcessor<'sdk> = Sdk::image_processor;
    let _: fn(&Device<'static>) -> DeviceState = Device::<'static>::state;
    let _: fn(&mut Device<'static>) -> Result<()> = Device::<'static>::start;
    let _: fn(&mut Device<'static>, Duration) -> Result<OwnedFrame> = Device::<'static>::get_image;
    let _: fn(&mut Device<'static>) -> Result<()> = Device::<'static>::soft_trigger;
    let _: fn(&mut Device<'static>) -> Result<()> = Device::<'static>::stop;
    let _: fn(&mut Device<'static>, CallbackOptions) -> Result<Receiver<OwnedFrame>> =
        Device::<'static>::start_receiving;
    let _: fn(&mut Device<'static>, CallbackOptions, fn(OwnedFrame)) -> Result<CallbackWorker> =
        Device::<'static>::start_with_callback::<fn(OwnedFrame)>;
    let _: fn(&Device<'static>) -> Option<CallbackStats> = Device::<'static>::image_callback_stats;
    let _: for<'device> fn(
        &'device mut Device<'static>,
        CallbackOptions,
    ) -> Result<Receiver<DeviceException>> = Device::<'static>::exception_receiver;
    let _: for<'device> fn(
        &'device mut Device<'static>,
        CallbackOptions,
        fn(DeviceException),
    ) -> Result<CallbackWorker> = Device::<'static>::on_exception::<fn(DeviceException)>;
    let _: fn(&Device<'static>) -> Option<CallbackStats> =
        Device::<'static>::exception_callback_stats;
    let _: fn(&mut Device<'static>) = Device::<'static>::disable_exception_delivery;
    let _ = Device::clear_buffer;
    let _: fn(&mut Device<'static>, &[u8], &[u8]) -> Result<()> = Device::<'static>::download_file;
    let _: fn(&mut Device<'static>, &[u8], &[u8]) -> Result<()> = Device::<'static>::upload_file;
    let _: fn(&mut Device<'static>) -> Result<FileTransferStatus> =
        Device::<'static>::file_transfer_progress;
    let _: fn(&mut Device<'static>, Duration, Duration) -> Result<Option<FileProgress>> =
        Device::<'static>::wait_file_transfer;
    let _: fn(Device<'static>) -> Result<()> = Device::<'static>::close;
    let _ = ImageProcessor::depth_to_point_cloud;
    let _ = ImageProcessor::depth_to_round_point_cloud;
    let _ = ImageProcessor::convert;
    let _ = ImageProcessor::mosaic_depth;
    let _ = ImageProcessor::save;
    let _: ImageFileFormat = ImageFileFormat::Ply;
}

// 验证 callback 公开字段使用 owned 数据并公开完整计数，防止泄漏内部借用状态。
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

// 验证图像类型保留已知值和未知位，防止新版 SDK 类型值被截断。
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

// 验证 OwnedFrame 公开 owned payload 与标定元数据，防止 callback 数据依赖 SDK 缓冲区。
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
