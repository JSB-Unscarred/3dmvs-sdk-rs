use std::sync::Arc;

use crate::driver::DriverError;
use crate::error::Error;
use crate::runtime::{Gate, Runtime};

use super::mock_driver::MockDriver;

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
