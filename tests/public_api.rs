use std::net::Ipv4Addr;

use mv3d_lp::{Device, Frame, ImageCalibration, ImageProcessor, ImageType, Result, Sdk};

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

// 验证 Frame 可公开构造且 Clone 深拷贝 payload。
#[test]
fn frame_is_publicly_constructible_and_cloneable() {
    let mut frame = Frame {
        image_type: ImageType::MONO8,
        width: 1,
        height: 1,
        data: vec![1],
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        calibration: ImageCalibration::default(),
    };

    let cloned = frame.clone();
    frame.data[0] = 2;
    assert_eq!(cloned.data, [1]);
}
