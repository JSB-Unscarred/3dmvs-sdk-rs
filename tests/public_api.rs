use std::net::Ipv4Addr;

use mv3d_lp::{
    Device, Frame, Image, ImageCalibration, ImageProcessor, ImageRef, ImageType, Result, Sdk,
};

const DATA: [u8; 2] = [1, 2];
const INTENSITY_DATA: [u8; 2] = [3, 4];
const EXPOSURE_TIMESTAMPS: [i64; 1] = [5];

// 验证公开生命周期返回 owned 类型，防止重新引入 Sdk 借用。
#[test]
fn public_lifecycle_uses_owned_types() {
    let _: fn() -> Result<Sdk> = Sdk::initialize;
    let _: fn(&Sdk, Ipv4Addr) -> Result<Device> = Sdk::open_by_ip;
    let _: fn(&Sdk, &mv3d_lp::SerialNumber) -> Result<Device> = Sdk::open_by_serial;
    let _: fn(&Sdk) -> ImageProcessor = Sdk::image_processor;
    let _: fn(Device) -> Result<()> = Device::close;
    let _: fn(&Sdk) -> Result<()> = Sdk::shutdown;
}

// 验证公开构造与 Clone 都深拷贝三类 payload，防止共享可变缓冲区。
#[test]
fn owned_outputs_copy_payloads() {
    let mut data = DATA;
    let mut intensity_data = INTENSITY_DATA;
    let mut exposure_timestamps = EXPOSURE_TIMESTAMPS;
    let image_ref = ImageRef {
        image_type: ImageType::MONO8,
        width: 2,
        height: 1,
        data: &data,
        intensity_data: Some(&intensity_data),
        exposure_timestamps: Some(&exposure_timestamps),
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        calibration: ImageCalibration::default(),
    };
    let mut frame = Frame::from_image_ref(image_ref);
    let mut image = Image::from_image_ref(image_ref);

    data.fill(0);
    intensity_data.fill(0);
    exposure_timestamps.fill(0);
    assert_payloads(frame.as_image_ref());
    assert_payloads(image.as_image_ref());

    let cloned_frame = frame.clone();
    let cloned_image = image.clone();
    frame.data.fill(0);
    frame.intensity_data.as_mut().unwrap().fill(0);
    frame.exposure_timestamps.as_mut().unwrap().fill(0);
    image.data.fill(0);
    image.intensity_data.as_mut().unwrap().fill(0);
    image.exposure_timestamps.as_mut().unwrap().fill(0);
    assert_payloads(cloned_frame.as_image_ref());
    assert_payloads(cloned_image.as_image_ref());
}

fn assert_payloads(image: ImageRef<'_>) {
    assert_eq!(image.data, DATA);
    assert_eq!(image.intensity_data, Some(INTENSITY_DATA.as_slice()));
    assert_eq!(
        image.exposure_timestamps,
        Some(EXPOSURE_TIMESTAMPS.as_slice())
    );
}
