use mv3d_lp::{Error, Operation, SdkError, StatusCode};

#[test]
fn strict_initializer_is_public() {
    let _: fn() -> mv3d_lp::Result<mv3d_lp::Sdk> = mv3d_lp::Sdk::initialize_strict;
}

#[test]
fn degraded_process_sdk_state_has_a_distinct_public_error() {
    let error = Error::RuntimeDegraded;

    assert!(error.to_string().contains("cannot open devices"));
}

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

#[test]
fn operation_names_use_the_vendor_symbols() {
    assert_eq!(Operation::GetImage.sdk_name(), "MV3D_LP_GetImage");
    assert_eq!(
        Operation::FileAccessRead.sdk_name(),
        "MV3D_LP_FileAccessRead"
    );
    assert_eq!(
        Operation::FileAccessWrite.sdk_name(),
        "MV3D_LP_FileAccessWrite"
    );
    assert_eq!(
        Operation::GetFileAccessProgress.sdk_name(),
        "MV3D_LP_GetFileAccessProgress"
    );
    assert_eq!(Operation::DisplayImage.sdk_name(), "MV3D_LP_DisplayImage");
}
