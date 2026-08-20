#![cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]

use std::env;

use mv3d_lp::{Sdk, SerialNumber};

const SERIAL_ENV: &str = "MV3D_LP_TEST_SERIAL";

// 验证真实硬件的最短 public API 数据流；手工提供序列号后单独运行。
#[test]
#[ignore]
fn native_pull_smoke() {
    let serial = env::var(SERIAL_ENV)
        .expect("MV3D_LP_TEST_SERIAL must contain the target device serial number");
    let serial = SerialNumber::new(serial).expect("valid device serial number");

    let sdk = Sdk::initialize().expect("initialize the installed SDK");
    let mut device = sdk
        .open_by_serial(&serial)
        .expect("open the configured device");
    device.start().expect("start acquisition");
    let image = device.get_image(10_000).expect("pull one frame");
    assert!(image.valid);
    device.stop().expect("stop acquisition");
    device.close().expect("close the device");
    sdk.shutdown().expect("finalize the SDK");
}
