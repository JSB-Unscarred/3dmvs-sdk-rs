use std::net::Ipv4Addr;

use crate::driver::DriverError;
use crate::error::Error;

use super::mock_driver::{Call, MockDriver, active_runtime};

// 验证 cleanup 的 Stop 失败后仍执行 Close，成功释放 handle 后可 Finalize。
#[test]
fn failed_cleanup_stop_still_closes_and_finalizes() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();

    let error = device.close().unwrap_err();
    assert!(error.stop.is_some());
    assert!(error.close.is_none());
    runtime.shutdown().unwrap();

    assert_eq!(
        &mock.calls()[3..],
        [Call::Start, Call::Stop, Call::Close, Call::Finalize]
    );
}

// 验证 Close 与 Finalize 失败后都可由既有 owner 路径重试。
#[test]
fn finalization_retries_after_device_close_and_sdk_failure() {
    let mock = MockDriver::new();
    mock.push_close(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    mock.push_finalize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices { live_handles: 1 })
    ));
    let error = device.close().unwrap_err();
    assert!(error.close.is_some());
    assert!(matches!(runtime.shutdown(), Err(Error::Sdk { .. })));
    runtime.shutdown().unwrap();

    assert_eq!(
        mock.calls(),
        [
            Call::Version,
            Call::Initialize,
            Call::OpenByIp,
            Call::Close,
            Call::Close,
            Call::Finalize,
            Call::Finalize,
        ]
    );
}
