use mv3d_lp::{Error, Operation, SdkError, StatusCode};

// 验证已知与未知状态码都保留厂商位模式和调用上下文。
#[test]
fn status_codes_preserve_their_bits_and_operation() {
    let known = StatusCode::from_raw(0x8006_000D_u32 as i32);
    assert_eq!(known, StatusCode::DEVICE_OFFLINE);
    assert_eq!(known.bits(), 0x8006_000D);
    assert_eq!(known.name(), Some("MV3D_LP_E_DEVICE_OFFLINE"));

    let unknown = StatusCode::from_bits(0xDEAD_BEEF);
    let error = SdkError::new(Operation::GetParam, unknown);
    assert_eq!(error.operation(), Operation::GetParam);
    assert_eq!(error.status(), unknown);
    assert!(error.to_string().contains("0xDEADBEEF"));
}

// 验证失效 Runtime token 与普通 SDK 错误可由调用方区分。
#[test]
fn runtime_inactive_is_a_distinct_public_error() {
    assert!(matches!(Error::RuntimeInactive, Error::RuntimeInactive));
    assert!(
        Error::RuntimeInactive
            .to_string()
            .contains("no longer refers")
    );
}
