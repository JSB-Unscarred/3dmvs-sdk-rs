//! 真机数据流测试：需要 3DMVS 与一台专用测试设备，运行方式：
//!
//! ```text
//! $env:MV3D_LP_TEST_SERIAL = "<设备序列号>"
//! cargo test --test hardware_smoke -- --ignored
//! ```
//!
//! `cargo test --workspace` 会启动本测试程序，没有 `Mv3dLp.dll` 时程序无法加载；
//! 未安装 SDK 的机器改用 `cargo test --workspace --lib --test thread_traits`。

use std::error::Error;
use std::ffi::CString;
use std::sync::mpsc;
use std::time::Duration;

use mv3d_lp::{CallbackGrabbing, Sdk};

const TIMEOUT: Duration = Duration::from_secs(10);

// pull（借用设备）与 callback（按值持有设备）两条取流链，以及显式清理；LPSDK 不能注销 image callback，所以先 pull 后 callback。
#[test]
#[ignore = "requires a dedicated device and MV3D_LP_TEST_SERIAL"]
fn pull_and_callback_grabbing() -> Result<(), Box<dyn Error>> {
    // 只操作专用测试设备，避免误用其它设备。
    let serial = CString::new(std::env::var("MV3D_LP_TEST_SERIAL")?)?;
    let sdk = Sdk::new()?;
    let mut device = sdk.open_by_serial(&serial)?;

    let grabbing = device.start_grabbing()?;
    let image = grabbing.get_image(Some(TIMEOUT))?;
    assert!(image.valid && !image.data.is_empty());
    grabbing.stop().1?;

    // callback 守卫按值持有设备，stop 后交还。
    let (sender, receiver) = mpsc::sync_channel(1);
    let grabbing = CallbackGrabbing::start(device, move |image| {
        let _ = sender.try_send(image);
    })
    .map_err(|(_, error)| error)?;
    assert!(!receiver.recv_timeout(TIMEOUT)?.data.is_empty());
    let (device, result) = grabbing.stop();
    result?;

    device.close()?;
    Ok(())
}
