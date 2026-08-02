use std::sync::Arc;

use crate::driver::DriverError;
use crate::error::{ContractViolation, Error, Operation};
use crate::runtime::{Gate, Runtime};

use super::mock_driver::{FfiOp, MockDriver};

#[test]
fn runtime_allows_one_active_instance_and_reinitializes_after_finalize() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();

    let second = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));
    assert!(matches!(second, Err(Error::RuntimeAlreadyActive)));

    runtime.shutdown().unwrap();
    let third = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    third.shutdown().unwrap();
    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "finalize",
            "version",
            "initialize",
            "finalize",
        ]
    );
}

#[test]
fn initialization_failure_returns_the_gate_to_fresh_for_retry() {
    let mock = MockDriver::new();
    mock.push_initialize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let gate = Arc::new(Gate::new());

    let first = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));
    assert!(matches!(first, Err(Error::Sdk { .. })));
    let second = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    second.shutdown().unwrap();
    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "finalize",
            "version",
            "initialize",
            "finalize"
        ]
    );
}

#[test]
fn initialization_cleanup_failure_degrades_the_process_sdk_state() {
    let mock = MockDriver::new();
    mock.push_initialize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    mock.push_finalize(Err(DriverError::Status(0x8006_0000_u32 as i32)));
    let gate = Arc::new(Gate::new());

    let first = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));
    assert!(matches!(
        first,
        Err(Error::Sdk {
            operation: Operation::Initialize,
            ..
        })
    ));
    let retry = Runtime::initialize_with(Box::new(mock.clone()), gate);
    assert!(matches!(retry, Err(Error::RuntimeDegraded)));
    assert_eq!(mock.logs(), ["version", "initialize", "finalize"]);
}

#[test]
fn compatible_build_version_initializes_by_default() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.3.4".to_vec()));
    let gate = Arc::new(Gate::new());

    let runtime = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();

    assert_eq!(runtime.version_bytes(), b"1.3.3.4");
    runtime.shutdown().unwrap();
    assert_eq!(mock.logs(), ["version", "initialize", "finalize"]);
}

#[test]
fn version_below_compatible_range_is_rejected_before_initialization() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.3.2".to_vec()));
    let gate = Arc::new(Gate::new());

    let result = Runtime::initialize_with(Box::new(mock.clone()), gate);

    assert!(matches!(result, Err(Error::IncompatibleSdkVersion { .. })));
    assert_eq!(mock.logs(), ["version"]);
}

#[test]
fn compatible_upper_bound_prevents_initialization_without_poisoning_the_gate() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.4.0".to_vec()));
    let gate = Arc::new(Gate::new());
    let result = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));

    let Err(Error::IncompatibleSdkVersion {
        minimum,
        maximum_exclusive,
        actual,
    }) = result
    else {
        panic!("the upper bound must be rejected before SDK initialization");
    };
    assert_eq!(minimum, b"1.3.3.3");
    assert_eq!(maximum_exclusive, Some(b"1.3.4.0".as_slice()));
    assert_eq!(actual, b"1.3.4.0");
    mock.set_version(Ok(b"1.3.3.3".to_vec()));
    let retry = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    retry.shutdown().unwrap();
    assert_eq!(
        mock.logs(),
        ["version", "version", "initialize", "finalize"]
    );
}

#[test]
fn strict_version_rejects_compatible_build_and_leaves_the_gate_fresh() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.3.4".to_vec()));
    let gate = Arc::new(Gate::new());
    let result = Runtime::initialize_with_strict(Box::new(mock.clone()), Arc::clone(&gate));

    let Err(Error::IncompatibleSdkVersion {
        minimum,
        maximum_exclusive,
        actual,
    }) = result
    else {
        panic!("strict initialization must reject a non-audited compatible build");
    };
    assert_eq!(minimum, b"1.3.3.3");
    assert_eq!(maximum_exclusive, None);
    assert_eq!(actual, b"1.3.3.4");

    mock.set_version(Ok(b"1.3.3.3".to_vec()));
    let retry = Runtime::initialize_with_strict(Box::new(mock.clone()), gate).unwrap();
    retry.shutdown().unwrap();
    assert_eq!(
        mock.logs(),
        ["version", "version", "initialize", "finalize"]
    );
}

#[test]
fn strict_version_comparison_is_numeric() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"01.03.003.0003".to_vec()));
    let gate = Arc::new(Gate::new());

    let runtime = Runtime::initialize_with_strict(Box::new(mock.clone()), gate).unwrap();

    assert_eq!(runtime.version_bytes(), b"01.03.003.0003");
    runtime.shutdown().unwrap();
    assert_eq!(mock.logs(), ["version", "initialize", "finalize"]);
}

#[test]
fn version_query_failure_returns_the_gate_to_fresh_for_retry() {
    let mock = MockDriver::new();
    mock.set_version(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let gate = Arc::new(Gate::new());

    let first = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate));
    assert!(matches!(first, Err(Error::Sdk { .. })));
    mock.set_version(Ok(b"1.3.3.3".to_vec()));
    let retry = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    retry.shutdown().unwrap();
    assert_eq!(
        mock.logs(),
        ["version", "version", "initialize", "finalize"]
    );
}

#[test]
fn finalize_failure_degrades_the_process_sdk_state() {
    let mock = MockDriver::new();
    mock.push_finalize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock), Arc::clone(&gate)).unwrap();

    assert!(matches!(runtime.shutdown(), Err(Error::Sdk { .. })));
    let retry_mock = MockDriver::new();
    let retry = Runtime::initialize_with(Box::new(retry_mock.clone()), gate);
    assert!(matches!(retry, Err(Error::RuntimeDegraded)));
    assert!(retry_mock.operations().is_empty());
}

#[test]
fn handle_count_overflow_degrades_the_process_sdk_state() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();
    runtime.set_live_handles_for_test(usize::MAX);

    assert!(matches!(
        runtime.open_by_ip("192.0.2.1".parse().unwrap()),
        Err(Error::ContractViolation {
            operation: Operation::OpenDeviceByIp,
            kind: ContractViolation::HandleCountOverflow,
        })
    ));
    assert_eq!(runtime.device_count_hint().unwrap(), 0);
    assert!(matches!(
        runtime.open_by_serial(b"SECOND"),
        Err(Error::RuntimeDegraded)
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
        [
            FfiOp::GetVersion,
            FfiOp::Initialize,
            FfiOp::OpenDeviceByIp,
            FfiOp::GetDeviceNumber,
        ]
    );
}

#[test]
fn handle_count_underflow_degrades_the_process_sdk_state() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    runtime.set_live_handles_for_test(0);

    device.close().unwrap();
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
        mock.operations(),
        [
            FfiOp::GetVersion,
            FfiOp::Initialize,
            FfiOp::OpenDeviceByIp,
            FfiOp::CloseDevice,
            FfiOp::GetDeviceNumber,
        ]
    );
}
