use std::sync::Arc;

use crate::driver::DriverError;
use crate::error::{Error, Operation};
use crate::runtime::{Gate, Runtime};

use super::mock_driver::MockDriver;

// 验证 Active session 复用 core，重启后旧 token 不能调用新的 native session。
#[test]
fn runtime_reuses_the_active_session_and_reinitializes_after_finalize() {
    let mock = MockDriver::new();
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();

    let second = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();
    assert_eq!(mock.logs(), ["version", "initialize"]);

    runtime.shutdown().unwrap();
    let third = Runtime::initialize_with(Box::new(mock.clone()), gate).unwrap();
    assert!(matches!(
        second.device_count_hint(),
        Err(Error::RuntimeInactive)
    ));
    assert!(matches!(second.shutdown(), Err(Error::RuntimeInactive)));
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

// 验证 initialize 失败后 gate 回到 Fresh，防止可重试错误永久锁死进程状态。
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

// 验证初始化清理失败使进程进入 Degraded，防止在不确定全局状态上继续初始化。
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

// 验证兼容版本可通过默认初始化，防止严格基线误用于普通入口。
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

// 验证低于兼容范围的版本在 initialize 前被拒绝，防止不匹配 ABI 进入 SDK。
#[test]
fn version_below_compatible_range_is_rejected_before_initialization() {
    let mock = MockDriver::new();
    mock.set_version(Ok(b"1.3.3.2".to_vec()));
    let gate = Arc::new(Gate::new());

    let result = Runtime::initialize_with(Box::new(mock.clone()), gate);

    assert!(matches!(result, Err(Error::IncompatibleSdkVersion { .. })));
    assert_eq!(mock.logs(), ["version"]);
}

// 验证兼容上界阻止初始化且保持 gate 可重试，防止新版 ABI 被误接受或污染状态。
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

// 验证 strict 入口拒绝非基线构建并保持 Fresh，防止验收失败阻断后续正确初始化。
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

// 验证版本按数值段比较，防止字典序误判多位版本号。
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

// 验证版本查询失败后 gate 回到 Fresh，防止预初始化错误污染生命周期。
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

// 验证 finalize 失败使进程进入 Degraded，防止在不确定全局状态上重启 SDK。
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
