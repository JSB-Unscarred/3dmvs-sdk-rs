use std::net::Ipv4Addr;

use mv3d_lp::{Device, ImageProcessor, Result, Sdk};

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
