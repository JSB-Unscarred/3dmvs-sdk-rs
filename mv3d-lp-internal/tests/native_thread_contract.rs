#![cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]

use std::env;

use mv3d_lp_internal::Runtime;

const SERIAL_ENV: &str = "MV3D_LP_TEST_SERIAL";

// 验证真实硬件的最短标准数据流；手工提供序列号后单独运行。
#[test]
#[ignore]
fn native_pull_smoke() {
    let serial = env::var(SERIAL_ENV)
        .expect("MV3D_LP_TEST_SERIAL must contain the target device serial number");
    assert!(!serial.is_empty() && serial.is_ascii() && serial.len() <= 16);

    let runtime = Runtime::initialize().expect("initialize the compatible SDK");
    let mut device = runtime
        .open_by_serial(serial.as_bytes())
        .expect("open the configured device");
    device.start().expect("start acquisition");
    let frame = device.get_image(10_000).expect("pull one frame");
    assert!(frame.valid);
    device.stop().expect("stop acquisition");
    device.close().expect("close the device");
    runtime.shutdown().expect("finalize the SDK");
}
