use std::net::Ipv4Addr;

use crate::driver::DriverError;
use crate::error::{ContractViolation, Error};
use crate::opened_device::DeviceState;

use super::mock_driver::{MockDriver, active_runtime};

// 验证 start 与 stop 成功后才更新状态，防止 Rust 状态领先于 native 状态。
#[test]
fn start_and_stop_update_state_only_after_success() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert_eq!(device.state(), DeviceState::Open);
    let measurement = device.start().unwrap();
    assert_eq!(measurement.state(), DeviceState::Measuring);
    measurement.stop().unwrap();
    assert_eq!(device.state(), DeviceState::Open);
    device.close().unwrap();
    assert_eq!(
        mock.logs(),
        vec![
            "version",
            "initialize",
            "open_by_ip",
            "start",
            "stop",
            "close"
        ]
    );
}

// 验证 start 失败后设备保持 Open 并可重试，防止可恢复错误污染生命周期。
#[test]
fn failed_start_leaves_device_open_and_is_retryable() {
    let mock = MockDriver::new();
    mock.push_start(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert!(matches!(device.start(), Err(Error::Sdk { .. })));
    assert_eq!(device.state(), DeviceState::Open);
    device.clear_buffer().unwrap();
    device.start().unwrap().stop().unwrap();
    device.close().unwrap();
}

// 验证成功状态配合空 handle 被视为契约错误，防止构造无效 Device。
#[test]
fn successful_open_with_a_null_handle_is_a_contract_violation() {
    let mock = MockDriver::new();
    mock.configure_open_by_ip(None, Ok(()));
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.open_by_ip(Ipv4Addr::LOCALHOST),
        Err(Error::ContractViolation {
            kind: ContractViolation::NullHandleOnSuccess,
            ..
        })
    ));
    assert!(runtime.device_count_hint().is_ok());
    runtime.shutdown().unwrap();
    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "open_by_ip",
            "device_number",
            "finalize"
        ]
    );
}

// 验证 open 失败时返回的 handle 不进入有效状态，防止使用来源不确定的句柄。
#[test]
fn handle_returned_on_open_failure_is_never_treated_as_valid() {
    let mock = MockDriver::new();
    mock.configure_open_by_ip(Some(2), Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.open_by_ip(Ipv4Addr::LOCALHOST),
        Err(Error::OpenFailedWithHandle { .. })
    ));
    assert_eq!(runtime.device_count_hint().unwrap(), 0);
    assert!(matches!(
        runtime.open_by_serial(b"SECOND"),
        Err(Error::RuntimeDegraded)
    ));
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
    assert_eq!(
        mock.logs(),
        ["version", "initialize", "open_by_ip", "device_number"]
    );
}
