use std::net::Ipv4Addr;
use std::panic::{AssertUnwindSafe, catch_unwind};

use crate::driver::DriverError;
use crate::error::Error;

use super::mock_driver::{FfiOp, MockDriver, active_runtime};

#[test]
fn drop_stops_before_closing_a_measuring_device() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    {
        let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
        let measurement = device.start().unwrap();
        drop(measurement);
    }

    let log = mock.logs();
    let cleanup = &log[log.len() - 2..];
    assert_eq!(cleanup, ["stop", "close"]);
}

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
    assert!(matches!(
        runtime.device_count_hint(),
        Err(Error::RuntimeTerminal)
    ));
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
    assert!(!mock.logs().contains(&"finalize"));
}

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

#[test]
fn forgotten_device_prevents_finalize() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
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
}

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

#[test]
fn failed_start_is_conservatively_stopped_then_closed_before_finalize() {
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
            FfiOp::StopMeasure,
            FfiOp::CloseDevice,
            FfiOp::Finalize,
        ]
    );
    mock.assert_no_pending_failures();
}

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
