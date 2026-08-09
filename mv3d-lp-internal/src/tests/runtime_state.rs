use std::sync::Arc;

use crate::driver::DriverError;
use crate::error::Error;
use crate::runtime::{Gate, Runtime};

use super::mock_driver::{Call, MockDriver};

// 验证初始化失败可重试，且同一 active session 不重复初始化。
#[test]
fn initialization_is_retryable_and_active_session_is_reused() {
    let mock = MockDriver::new();
    mock.push_initialize(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let gate = Arc::new(Gate::new());

    assert!(matches!(
        Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)),
        Err(Error::Sdk { .. })
    ));
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();
    let second = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    runtime.shutdown().unwrap();

    assert!(matches!(
        second.device_count_hint(),
        Err(Error::RuntimeInactive)
    ));
    assert_eq!(
        mock.calls(),
        [
            Call::Version,
            Call::Initialize,
            Call::Version,
            Call::Initialize,
            Call::Finalize,
        ]
    );
}

// 验证兼容范围外版本不会污染 gate，随后可用审计版本初始化。
#[test]
fn version_rejection_leaves_initialization_retryable() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.4.0".to_vec()));
    let gate = Arc::new(Gate::new());

    assert!(matches!(
        Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)),
        Err(Error::IncompatibleSdkVersion { .. })
    ));
    mock.set_version(Ok(b"1.3.3.3".to_vec()));
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    runtime.shutdown().unwrap();

    assert_eq!(
        mock.calls(),
        [
            Call::Version,
            Call::Version,
            Call::Initialize,
            Call::Finalize
        ]
    );
}
