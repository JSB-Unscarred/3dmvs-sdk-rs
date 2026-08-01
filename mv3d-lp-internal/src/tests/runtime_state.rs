use std::sync::Arc;

use crate::driver::DriverError;
use crate::error::{ContractViolation, Error};
use crate::runtime::{Gate, Runtime};

use super::mock_driver::{FfiOp, MockDriver};

#[test]
fn runtime_allows_one_initialization_and_then_becomes_terminal() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();

    let second = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));
    assert!(matches!(second, Err(Error::RuntimeAlreadyActive)));

    runtime.shutdown().unwrap();
    let third = Runtime::initialize_with(Box::new(mock.clone()), gate);
    assert!(matches!(third, Err(Error::RuntimeTerminal)));
    assert_eq!(mock.logs(), vec!["version", "initialize", "finalize"]);
}

#[test]
fn initialization_failure_is_terminal_and_is_not_retried() {
    let mock = MockDriver::new();
    mock.push_initialize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let gate = Arc::new(Gate::new());

    let first = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));
    assert!(matches!(first, Err(Error::Sdk { .. })));
    let second = Runtime::initialize_with(Box::new(mock.clone()), gate);
    assert!(matches!(second, Err(Error::RuntimeTerminal)));
    assert_eq!(mock.logs(), vec!["version", "initialize"]);
}

#[test]
fn incompatible_version_prevents_sdk_initialization() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.4.0".to_vec()));
    let gate = Arc::new(Gate::new());
    let result = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));

    assert!(matches!(result, Err(Error::IncompatibleSdkVersion { .. })));
    let retry = Runtime::initialize_with(Box::new(mock.clone()), gate);
    assert!(matches!(retry, Err(Error::RuntimeTerminal)));
    assert_eq!(mock.logs(), vec!["version"]);
}

#[test]
fn finalize_failure_still_makes_the_runtime_terminal() {
    let mock = MockDriver::new();
    mock.push_finalize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock), Arc::clone(&gate)).unwrap();

    assert!(matches!(runtime.shutdown(), Err(Error::Sdk { .. })));
    let retry = Runtime::initialize_with(Box::new(MockDriver::new()), gate);
    assert!(matches!(retry, Err(Error::RuntimeTerminal)));
}

#[test]
fn handle_count_overflow_after_open_fails_closed() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();
    runtime.set_live_handles_for_test(usize::MAX);

    assert!(matches!(
        runtime.open_by_ip("192.0.2.1".parse().unwrap()),
        Err(Error::ContractViolation {
            operation: "MV3D_LP_OpenDeviceByIP",
            kind: ContractViolation::HandleCountOverflow,
        })
    ));
    assert!(matches!(
        runtime.device_count_hint(),
        Err(Error::RuntimeTerminal)
    ));
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: usize::MAX,
            teardown_uncertain: true,
        })
    ));
    assert_eq!(
        mock.operations(),
        [FfiOp::GetVersion, FfiOp::Initialize, FfiOp::OpenDeviceByIp,]
    );
}

#[test]
fn handle_count_underflow_after_close_fails_closed() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    runtime.set_live_handles_for_test(0);

    device.close().unwrap();
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
    assert_eq!(
        mock.operations(),
        [
            FfiOp::GetVersion,
            FfiOp::Initialize,
            FfiOp::OpenDeviceByIp,
            FfiOp::CloseDevice,
        ]
    );
}
