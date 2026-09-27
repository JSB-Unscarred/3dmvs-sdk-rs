//! 真机数据流测试：需要 3DMVS SDK、专用测试设备与 `MV3D_LP_TEST_SERIAL`。

use std::error::Error;
use std::ffi::CString;
use std::sync::mpsc;
use std::time::Duration;

use mv3d_lp::Sdk;

const TIMEOUT: Duration = Duration::from_secs(10);

// pull 与 callback 两条取流链，以及显式清理；LPSDK 不能注销 image callback，所以先 pull 后 callback。
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
    grabbing.stop()?;

    let (sender, receiver) = mpsc::sync_channel(1);
    let grabbing = device.start_grabbing_with(move |image| {
        let _ = sender.try_send(image);
    })?;
    assert!(!receiver.recv_timeout(TIMEOUT)?.data.is_empty());
    grabbing.stop()?;

    device.close()?;
    Ok(())
}
