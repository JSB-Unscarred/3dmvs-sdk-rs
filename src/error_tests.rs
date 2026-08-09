use mv3d_lp_internal::{
    ContractViolation as InternalContract, Error as InternalError, InvalidInput as InternalInput,
    Operation as InternalOperation,
};

use super::{
    ContractViolation, Error, InputViolation, Operation, SdkError, SdkText, SdkVersion, StatusCode,
};

// 覆盖错误边界的四种公开形状，避免逐项复刻两个 error enum。
#[test]
fn representative_internal_errors_map_to_public_shapes() {
    assert_eq!(
        Error::map_internal_error(InternalError::Sdk {
            operation: InternalOperation::GetImage,
            status: 0x8006_0006_u32 as i32,
        }),
        Error::Sdk(SdkError::new(Operation::GetImage, StatusCode::NO_DATA))
    );
    assert_eq!(
        Error::map_internal_error(InternalError::InvalidState {
            operation: InternalOperation::GetImage,
            state: "open",
        }),
        Error::InvalidState {
            operation: Operation::GetImage,
            expected: "measuring",
            actual: "open",
        }
    );
    assert_eq!(
        Error::map_internal_error(InternalError::InvalidInput {
            operation: InternalOperation::SetParam,
            kind: InternalInput::InteriorNul,
        }),
        Error::InvalidInput {
            field: "MV3D_LP_SetParam",
            violation: InputViolation::InteriorNul,
        }
    );
    assert_eq!(
        Error::map_internal_error(InternalError::ContractViolation {
            operation: InternalOperation::GetImage,
            kind: InternalContract::NullPointerWithLength {
                field: "image data",
                length: 8,
            },
        }),
        Error::ContractViolation {
            operation: Operation::GetImage,
            violation: ContractViolation::NullPointerWithLength {
                field: "image data",
                length: 8,
            },
        }
    );
}

// 版本映射需要重新解析字节，单独覆盖有效文本与畸形文本。
#[test]
fn incompatible_version_mapping_preserves_diagnostics() {
    let error = Error::map_internal_error(InternalError::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: b"1.4.0.0",
        actual: b"01.4.0.0".to_vec(),
    });
    assert_eq!(
        error,
        Error::IncompatibleSdkVersion {
            minimum: SdkVersion::new(1, 3, 3, 3),
            maximum_exclusive: SdkVersion::new(1, 4, 0, 0),
            actual: SdkVersion::new(1, 4, 0, 0),
            actual_text: SdkText::new("01.4.0.0").unwrap(),
        }
    );

    let malformed = Error::map_internal_error(InternalError::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: b"1.4.0.0",
        actual: format!("{}1.3.3.3", "0".repeat(SdkText::MAX_LEN)).into_bytes(),
    });
    assert_eq!(
        malformed,
        Error::ContractViolation {
            operation: Operation::GetVersion,
            violation: ContractViolation::InvalidValue {
                field: "SDK version",
            },
        }
    );
}
