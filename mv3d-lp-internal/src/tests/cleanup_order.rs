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
    let measurement = device.start().unwrap();
    std::mem::forget(measurement);

    let error = device.close().unwrap_err();
    assert!(error.stop.is_some());
    assert!(error.close.is_some());
    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
    assert_eq!(runtime.device_count_hint().unwrap(), 0);
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
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
    let measurement = second.start().unwrap();
    std::mem::forget(measurement);

    assert!(first.close().is_err());
    second.clear_buffer().unwrap();
    second.close().unwrap();
    assert!(matches!(
        runtime.open_by_serial(b"THIRD"),
        Err(Error::RuntimeDegraded)
    ));
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));

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

// 验证遗忘 Device 时拒绝 finalize，防止 SDK 在活跃 handle 存续期间卸载。
#[test]
fn forgotten_device_prevents_finalize() {
    let mock = MockDriver::new();
    let (runtime, gate) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    std::mem::forget(device);

    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 1,
            teardown_uncertain: false,
        })
    ));
    assert!(!mock.logs().contains(&"finalize"));

    let retry_mock = MockDriver::new();
    let retry = crate::runtime::Runtime::initialize_with(Box::new(retry_mock.clone()), gate);
    assert!(matches!(retry, Err(Error::RuntimeDegraded)));
    assert!(retry_mock.operations().is_empty());
}

// 验证隐式 Drop 严格执行 stop、close、finalize 一次，防止遗漏或重复清理。
#[test]
fn implicit_drop_has_one_exact_stop_close_finalize_sequence() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    drop(device.start().unwrap());
    drop(device);
    drop(runtime);

    assert_eq!(
        mock.operations(),
        [
            FfiOp::GetVersion,
            FfiOp::Initialize,
            FfiOp::OpenDeviceByIp,
            FfiOp::StartMeasure,
            FfiOp::StopMeasure,
            FfiOp::CloseDevice,
            FfiOp::Finalize,
        ]
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
    let measurement = device.start().unwrap();
    std::mem::forget(measurement);
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
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));

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

// 验证 finalize 等待所有独立设备关闭，防止任一活跃 handle 被提前失效。
#[test]
fn finalize_waits_for_every_distinct_device_close() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let first = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let second = runtime.open_by_serial(b"SECOND").unwrap();

    second.close().unwrap();
    assert!(!mock.operations().contains(&FfiOp::Finalize));
    first.close().unwrap();
    assert!(!mock.operations().contains(&FfiOp::Finalize));
    runtime.shutdown().unwrap();

    assert_eq!(
        mock.opened_handles(),
        [(FfiOp::OpenDeviceByIp, 1), (FfiOp::OpenDeviceBySerial, 2)]
    );
    assert_eq!(mock.closed_handles(), [2, 1]);
    assert_eq!(
        &mock.operations()[2..],
        [
            FfiOp::OpenDeviceByIp,
            FfiOp::OpenDeviceBySerial,
            FfiOp::CloseDevice,
            FfiOp::CloseDevice,
            FfiOp::Finalize,
        ]
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
        let measurement = device.start().unwrap();
        std::mem::forget(measurement);
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

// 验证 finalize 失败在 Runtime Drop 中不 unwind 且不重试，防止重复终结不确定状态。
#[test]
fn finalize_status_failure_never_unwinds_from_runtime_drop_or_retries() {
    let mock = MockDriver::new();
    mock.fail_next(FfiOp::Finalize, DriverError::Status(0x8006_0005_u32 as i32));

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let (runtime, _) = active_runtime(&mock);
        drop(runtime);
    }));

    assert!(outcome.is_ok());
    assert_eq!(
        mock.operations(),
        [FfiOp::GetVersion, FfiOp::Initialize, FfiOp::Finalize]
    );
    mock.assert_no_pending_failures();
}
