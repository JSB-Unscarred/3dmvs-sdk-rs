use mv3d_lp::{Operation, SdkError, StatusCode};

#[test]
fn known_high_bit_status_preserves_its_exact_bits() {
    let status = StatusCode::from_raw(0x8006_000D_u32 as i32);

    assert_eq!(status, StatusCode::DEVICE_OFFLINE);
    assert_eq!(status.bits(), 0x8006_000D);
    assert_eq!(status.raw(), 0x8006_000D_u32 as i32);
    assert_eq!(status.name(), Some("MV3D_LP_E_DEVICE_OFFLINE"));
}

#[test]
fn unknown_status_is_retained_with_its_operation() {
    let status = StatusCode::from_bits(0xDEAD_BEEF);
    let error = SdkError::new(Operation::GetParam, status);

    assert_eq!(status.name(), None);
    assert_eq!(error.operation(), Operation::GetParam);
    assert_eq!(error.status().bits(), 0xDEAD_BEEF);
    assert!(error.to_string().contains("0xDEADBEEF"));
}
