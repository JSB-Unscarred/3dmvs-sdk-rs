//! 真机数据流测试：需要 3DMVS SDK、专用设备与 `MV3D_LP_TEST_SERIAL`。

use std::error::Error;
use std::ffi::CString;
use std::time::Duration;

use mv3d_lp::Sdk;

// pull 采集的最短数据流与显式清理。
#[test]
#[ignore = "requires the 3DMVS SDK and MV3D_LP_TEST_SERIAL"]
fn real_device_pull_flow() -> Result<(), Box<dyn Error>> {
    let serial = CString::new(std::env::var("MV3D_LP_TEST_SERIAL")?)?;

    let sdk = Sdk::new()?;
    let mut device = sdk.open_by_serial(&serial)?;

    let grabbing = device.start_grabbing()?;
    let image = grabbing.get_image(Some(Duration::from_secs(10)))?;
    assert!(image.valid && !image.data.is_empty());
    grabbing.stop()?;

    device.close()?;
    Ok(())
}
