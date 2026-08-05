use mv3d_lp::{Error, Operation, SdkError, StatusCode};

// 验证严格初始化入口保持公开，防止 ABI 基线检查从公开 API 中消失。
#[test]
fn strict_initializer_is_public() {
    let _: fn() -> mv3d_lp::Result<mv3d_lp::Sdk> = mv3d_lp::Sdk::initialize_strict;
}

// 验证 degraded 进程状态使用独立公开错误，防止调用方误判为普通 SDK 失败。
#[test]
fn degraded_process_sdk_state_has_a_distinct_public_error() {
    let error = Error::RuntimeDegraded;

    assert!(error.to_string().contains("cannot open devices"));
}

// 验证高位状态码按原始位模式保存，防止有符号转换改变厂商错误值。
#[test]
fn known_high_bit_status_preserves_its_exact_bits() {
    let status = StatusCode::from_raw(0x8006_000D_u32 as i32);

    assert_eq!(status, StatusCode::DEVICE_OFFLINE);
    assert_eq!(status.bits(), 0x8006_000D);
    assert_eq!(status.raw(), 0x8006_000D_u32 as i32);
    assert_eq!(status.name(), Some("MV3D_LP_E_DEVICE_OFFLINE"));
}

// 验证未知状态码与操作上下文完整保留，防止新版 SDK 错误信息丢失。
#[test]
fn unknown_status_is_retained_with_its_operation() {
    let status = StatusCode::from_bits(0xDEAD_BEEF);
    let error = SdkError::new(Operation::GetParam, status);

    assert_eq!(status.name(), None);
    assert_eq!(error.operation(), Operation::GetParam);
    assert_eq!(error.status().bits(), 0xDEAD_BEEF);
    assert!(error.to_string().contains("0xDEADBEEF"));
}
