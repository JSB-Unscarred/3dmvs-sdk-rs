use std::net::Ipv4Addr;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::driver::DriverError;
use crate::error::Error;

use super::mock_driver::{FfiOp, MockDriver, active_runtime};

// 验证 cleanup stop 失败后仍尝试 close，防止单步失败跳过后续资源释放。
#[test]
fn close_is_attempted_even_when_cleanup_stop_fails() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    mock.push_close(Err(DriverError::Status(0x8006_0000_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();

    let error = device.close().unwrap_err();
    assert!(error.stop.is_some());
    assert!(error.close.is_some());
    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
    assert_eq!(runtime.device_count_hint().unwrap(), 0);
    assert!(matches!(runtime.shutdown(), Err(Error::RuntimeDegraded)));
    assert!(!mock.logs().contains(&"finalize"));
}

// 验证一个设备 close 失败不阻塞健康设备清理，防止进程故障扩大到现有 handle。
#[test]
fn failed_close_does_not_block_a_healthy_device() {
    let mock = MockDriver::new();
    mock.fail_next(
        FfiOp::CloseDevice,
        DriverError::Status(0x8006_0000_u32 as i32),
    );
    let (runtime, _) = active_runtime(&mock);
    let first = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut second = runtime.open_by_serial(b"SECOND").unwrap();
    second.start().unwrap();

    assert!(first.close().is_err());
    second.clear_buffer().unwrap();
    second.close().unwrap();
    assert!(matches!(
        runtime.open_by_serial(b"THIRD"),
        Err(Error::RuntimeDegraded)
    ));
    assert!(matches!(runtime.shutdown(), Err(Error::RuntimeDegraded)));

    assert_eq!(mock.closed_handles(), [1, 2]);
    assert_eq!(
        mock.operations()
            .iter()
            .filter(|operation| **operation == FfiOp::ClearDataBuffer)
            .count(),
        1
    );
    assert_eq!(
        mock.operations()
            .iter()
            .filter(|operation| **operation == FfiOp::StopMeasure)
            .count(),
        1
    );
    assert_eq!(
        mock.operations()
            .iter()
            .filter(|operation| **operation == FfiOp::OpenDeviceBySerial)
            .count(),
        1
    );
    assert!(!mock.operations().contains(&FfiOp::Finalize));
    mock.assert_no_pending_failures();
}

// 验证显式 close 成功后 Drop 不重复关闭，防止 native handle 被二次释放。
#[test]
fn explicit_close_is_not_repeated_by_drop() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.close().unwrap();
    runtime.shutdown().unwrap();

    assert_eq!(
        mock.logs()
            .iter()
            .filter(|entry| **entry == "close")
            .count(),
        1
    );
    assert_eq!(
        mock.logs()
            .iter()
            .filter(|entry| **entry == "finalize")
            .count(),
        1
    );
}

// 验证 start 失败后直接 close 再 finalize，防止对未启动采集调用 stop。
#[test]
fn failed_start_closes_without_stop_before_finalize() {
    let mock = MockDriver::new();
    mock.fail_next(
        FfiOp::StartMeasure,
        DriverError::Status(0x8006_0003_u32 as i32),
    );
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert!(device.start().is_err());
    device.close().unwrap();
    runtime.shutdown().unwrap();

    assert_eq!(
        &mock.operations()[2..],
        [
            FfiOp::OpenDeviceByIp,
            FfiOp::StartMeasure,
            FfiOp::CloseDevice,
            FfiOp::Finalize,
        ]
    );
    mock.assert_no_pending_failures();
}

// 验证 cleanup stop 失败但 close 成功时仍可 finalize，防止已释放 handle 阻塞退出。
#[test]
fn failed_cleanup_stop_still_closes_and_a_successful_close_allows_finalize() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();
    mock.fail_next(
        FfiOp::StopMeasure,
        DriverError::Status(0x8006_0003_u32 as i32),
    );

    let error = device.close().unwrap_err();
    assert!(error.stop.is_some());
    assert!(error.close.is_none());
    runtime.shutdown().unwrap();

    assert_eq!(
        &mock.operations()[3..],
        [
            FfiOp::StartMeasure,
            FfiOp::StopMeasure,
            FfiOp::CloseDevice,
            FfiOp::Finalize,
        ]
    );
    mock.assert_no_pending_failures();
}

// 验证 close 失败只消费一次并持续禁止 finalize，防止重试不确定 handle。
#[test]
fn failed_close_is_consumed_once_and_permanently_suppresses_finalize() {
    let mock = MockDriver::new();
    mock.fail_next(
        FfiOp::CloseDevice,
        DriverError::Status(0x8006_0000_u32 as i32),
    );
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let error = device.close().unwrap_err();
    assert!(error.stop.is_none());
    assert!(error.close.is_some());
    assert!(matches!(runtime.shutdown(), Err(Error::RuntimeDegraded)));

    assert_eq!(
        mock.operations()
            .iter()
            .filter(|operation| **operation == FfiOp::CloseDevice)
            .count(),
        1
    );
    assert!(!mock.operations().contains(&FfiOp::Finalize));
    mock.assert_no_pending_failures();
}

// 验证 live Device 暂时拒绝 finalize，关闭后可重新取得 token 完成 shutdown。
#[test]
fn finalize_can_retry_after_the_live_device_closes() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices { live_handles: 1 })
    ));
    device.close().unwrap();
    runtime.shutdown().unwrap();
    runtime.shutdown().unwrap();

    assert_eq!(
        &mock.operations()[2..],
        [FfiOp::OpenDeviceByIp, FfiOp::CloseDevice, FfiOp::Finalize,]
    );
}

// 验证 Drop 中的 stop 与 close 状态错误不触发 unwind，防止析构期间双重 panic。
#[test]
fn cleanup_status_failures_never_unwind_from_drop() {
    let mock = MockDriver::new();
    mock.fail_next(
        FfiOp::StopMeasure,
        DriverError::Status(0x8006_0003_u32 as i32),
    );
    mock.fail_next(
        FfiOp::CloseDevice,
        DriverError::Status(0x8006_0000_u32 as i32),
    );

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let (runtime, _) = active_runtime(&mock);
        let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
        device.start().unwrap();
        drop(device);
        drop(runtime);
    }));

    assert!(outcome.is_ok());
    assert_eq!(
        &mock.operations()[3..],
        [FfiOp::StartMeasure, FfiOp::StopMeasure, FfiOp::CloseDevice,]
    );
    assert!(!mock.operations().contains(&FfiOp::Finalize));
    mock.assert_no_pending_failures();
}
