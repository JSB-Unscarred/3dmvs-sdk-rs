use std::net::Ipv4Addr;

use mv3d_lp::{
    Camera, DeviceInfo, ImageType, IpConfiguration, Measurement, OwnedFrame, Result, Sdk, SdkText,
};

#[test]
fn public_lifecycle_and_control_methods_have_safe_signatures() {
    let _: fn() -> Result<Sdk> = Sdk::initialize;
    let _: for<'sdk> fn(&'sdk Sdk, Ipv4Addr) -> Result<Camera<'sdk>> = Sdk::open_by_ip;
    let _: for<'sdk> fn(&'sdk Sdk, &mv3d_lp::SerialNumber) -> Result<Camera<'sdk>> =
        Sdk::open_by_serial;
    let _: fn(Sdk) -> Result<()> = Sdk::shutdown;
    let _ = Camera::start;
    let _ = Camera::clear_buffer;
    let _ = Measurement::get_image;
    let _ = Measurement::soft_trigger;
    let _ = Measurement::clear_buffer;
    let _ = Measurement::stop;
}

#[test]
fn owned_output_types_can_move_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<DeviceInfo>();
    assert_send_sync::<SdkText>();
    assert_send_sync::<IpConfiguration>();
    assert_send_sync::<OwnedFrame>();
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
