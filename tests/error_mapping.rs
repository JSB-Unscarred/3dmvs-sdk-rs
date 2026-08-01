use mv3d_lp::{
    ContractViolation, Error, InputViolation, Operation, SdkError, SdkText, SdkVersion, StatusCode,
};

#[test]
fn strict_initializer_is_public() {
    let _: fn() -> mv3d_lp::Result<mv3d_lp::Sdk> = mv3d_lp::Sdk::initialize_strict;
}

#[test]
fn degraded_process_sdk_state_has_a_distinct_public_error() {
    let error: Error = mv3d_lp_internal::Error::RuntimeDegraded.into();

    assert_eq!(error, Error::RuntimeDegraded);
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
fn get_image_operation_uses_the_vendor_symbol() {
    assert_eq!(Operation::GetImage.sdk_name(), "MV3D_LP_GetImage");

    let error: Error = mv3d_lp_internal::Error::Sdk {
        operation: "MV3D_LP_GetImage",
        status: 0x8006_0006_u32 as i32,
    }
    .into();

    assert_eq!(
        error,
        Error::Sdk(SdkError::new(Operation::GetImage, StatusCode::NO_DATA))
    );
}

#[test]
fn incompatible_version_range_retains_the_original_sdk_text() {
    let error: Error = mv3d_lp_internal::Error::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: Some(b"1.4.0.0"),
        actual: b"01.4.0.0".to_vec(),
    }
    .into();

    assert_eq!(
        error,
        Error::IncompatibleSdkVersion {
            minimum: SdkVersion::new(1, 3, 3, 3),
            maximum_exclusive: Some(SdkVersion::new(1, 4, 0, 0)),
            actual: SdkVersion::new(1, 4, 0, 0),
            actual_text: SdkText::new("01.4.0.0").unwrap(),
        }
    );
    assert_eq!(
        error.to_string(),
        "incompatible SDK runtime version 01.4.0.0; expected a version in [1.3.3.3, 1.4.0.0)"
    );
}

#[test]
fn strict_incompatible_version_reports_the_exact_requirement() {
    let error: Error = mv3d_lp_internal::Error::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: None,
        actual: b"1.3.3.4".to_vec(),
    }
    .into();

    assert_eq!(
        error,
        Error::IncompatibleSdkVersion {
            minimum: SdkVersion::new(1, 3, 3, 3),
            maximum_exclusive: None,
            actual: SdkVersion::new(1, 3, 3, 4),
            actual_text: SdkText::new("1.3.3.4").unwrap(),
        }
    );
    assert_eq!(
        error.to_string(),
        "incompatible SDK runtime version 1.3.3.4; expected exactly 1.3.3.3"
    );
}

#[test]
fn malformed_incompatible_version_maps_without_panicking() {
    let error: Error = mv3d_lp_internal::Error::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: Some(b"1.4.0.0"),
        actual: format!("{}1.3.3.3", "0".repeat(SdkText::MAX_LEN)).into_bytes(),
    }
    .into();

    assert_eq!(
        error,
        Error::ContractViolation {
            operation: Operation::GetVersion,
            violation: ContractViolation::InvalidValue {
                field: "SDK version",
            },
        }
    );
}

#[test]
fn image_contract_violations_retain_lengths_and_operation() {
    let error: Error = mv3d_lp_internal::Error::ContractViolation {
        operation: "MV3D_LP_GetImage",
        kind: mv3d_lp_internal::ContractViolation::LengthMismatch {
            field: "image data",
            expected: 24,
            actual: 12,
        },
    }
    .into();

    assert_eq!(
        error,
        Error::ContractViolation {
            operation: Operation::GetImage,
            violation: ContractViolation::LengthMismatch {
                field: "image data",
                expected: 24,
                actual: 12,
            },
        }
    );
}

#[test]
fn remaining_image_contract_violations_map_without_losing_context() {
    let cases = [
        (
            mv3d_lp_internal::ContractViolation::NullPointerWithLength {
                field: "image data",
                length: 8,
            },
            ContractViolation::NullPointerWithLength {
                field: "image data",
                length: 8,
            },
        ),
        (
            mv3d_lp_internal::ContractViolation::LengthOverflow {
                field: "frame payload",
            },
            ContractViolation::LengthOverflow {
                field: "frame payload",
            },
        ),
        (
            mv3d_lp_internal::ContractViolation::OutputTooLarge {
                field: "frame payload",
                limit: 512,
                actual: 513,
            },
            ContractViolation::OutputTooLarge {
                field: "frame payload",
                limit: 512,
                actual: 513,
            },
        ),
        (
            mv3d_lp_internal::ContractViolation::InvalidImageValue {
                field: "valid flag",
            },
            ContractViolation::InvalidValue {
                field: "valid flag",
            },
        ),
    ];

    for (kind, violation) in cases {
        let error: Error = mv3d_lp_internal::Error::ContractViolation {
            operation: "MV3D_LP_GetImage",
            kind,
        }
        .into();

        assert_eq!(
            error,
            Error::ContractViolation {
                operation: Operation::GetImage,
                violation,
            }
        );
    }
}

#[test]
fn excessive_timeout_retains_the_exact_duration() {
    let error: Error = mv3d_lp_internal::Error::InvalidInput {
        operation: "timeout",
        kind: mv3d_lp_internal::InvalidInput::TimeoutTooLong {
            maximum_millis: u32::MAX - 1,
            actual_millis: u128::from(u32::MAX),
        },
    }
    .into();

    assert_eq!(
        error,
        Error::InvalidInput {
            field: "timeout",
            violation: InputViolation::TimeoutTooLong {
                maximum_millis: u32::MAX - 1,
                actual_millis: u128::from(u32::MAX),
            },
        }
    );
}

#[test]
fn allocation_failure_retains_its_operation() {
    let error: Error = mv3d_lp_internal::Error::AllocationFailed {
        operation: "MV3D_LP_GetImage",
        requested: 1024,
    }
    .into();

    assert_eq!(
        error,
        Error::AllocationFailed {
            operation: Operation::GetImage,
        }
    );
}

#[test]
fn file_transfer_operations_and_progress_contracts_map_exactly() {
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

    let cases = [
        (
            mv3d_lp_internal::ContractViolation::NegativeFileProgress {
                completed: -1,
                total: 10,
            },
            ContractViolation::NegativeFileProgress {
                completed: -1,
                total: 10,
            },
        ),
        (
            mv3d_lp_internal::ContractViolation::FileProgressExceedsTotal {
                completed: 11,
                total: 10,
            },
            ContractViolation::FileProgressExceedsTotal {
                completed: 11,
                total: 10,
            },
        ),
        (
            mv3d_lp_internal::ContractViolation::FileProgressRegressed {
                previous: 5,
                current: 4,
            },
            ContractViolation::FileProgressRegressed {
                previous: 5,
                current: 4,
            },
        ),
        (
            mv3d_lp_internal::ContractViolation::FileProgressTotalChanged {
                previous: 10,
                current: 4,
            },
            ContractViolation::FileProgressTotalChanged {
                previous: 10,
                current: 4,
            },
        ),
    ];

    for (kind, violation) in cases {
        let error: Error = mv3d_lp_internal::Error::ContractViolation {
            operation: "MV3D_LP_GetFileAccessProgress",
            kind,
        }
        .into();
        assert_eq!(
            error,
            Error::ContractViolation {
                operation: Operation::GetFileAccessProgress,
                violation,
            }
        );
    }
}

#[test]
fn device_cleanup_mapping_preserves_the_close_error() {
    let error: Error = mv3d_lp_internal::DeviceCleanupError {
        stop: None,
        close: Some(Box::new(mv3d_lp_internal::Error::Sdk {
            operation: "MV3D_LP_CloseDevice",
            status: 0x8006_0000_u32 as i32,
        })),
    }
    .into();

    assert_eq!(
        error,
        Error::DeviceCleanup {
            stop: None,
            close: Some(Box::new(Error::Sdk(SdkError::new(
                Operation::CloseDevice,
                StatusCode::INVALID_HANDLE,
            )))),
        }
    );
}
