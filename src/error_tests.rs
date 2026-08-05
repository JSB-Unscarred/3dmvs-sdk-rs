use std::error::Error as StdError;

use mv3d_lp_internal::{
    ContractViolation as InternalContract, Error as InternalError, InvalidInput as InternalInput,
    Operation as InternalOperation,
};

use super::{
    ContractViolation, Error, InputViolation, Operation, SdkError, SdkText, SdkVersion, StatusCode,
    map_internal_operation,
};

// 验证内部 Operation 完整映射到公开符号，防止新增 FFI 操作遗漏错误上下文。
#[test]
fn every_internal_operation_maps_to_the_public_operation_and_name() {
    let cases = [
        (InternalOperation::GetVersion, Operation::GetVersion),
        (InternalOperation::Initialize, Operation::Initialize),
        (InternalOperation::Finalize, Operation::Finalize),
        (
            InternalOperation::GetDeviceNumber,
            Operation::GetDeviceNumber,
        ),
        (InternalOperation::GetDeviceList, Operation::GetDeviceList),
        (InternalOperation::OpenDeviceByIp, Operation::OpenDeviceByIp),
        (InternalOperation::OpenDeviceBySn, Operation::OpenDeviceBySn),
        (InternalOperation::CloseDevice, Operation::CloseDevice),
        (InternalOperation::SetIpConfig, Operation::SetIpConfig),
        (InternalOperation::StartMeasure, Operation::StartMeasure),
        (InternalOperation::StopMeasure, Operation::StopMeasure),
        (InternalOperation::SoftTrigger, Operation::SoftTrigger),
        (
            InternalOperation::ClearDataBuffer,
            Operation::ClearDataBuffer,
        ),
        (InternalOperation::GetImage, Operation::GetImage),
        (
            InternalOperation::RegisterImageDataCallback,
            Operation::RegisterImageDataCallback,
        ),
        (
            InternalOperation::RegisterExceptionCallback,
            Operation::RegisterExceptionCallback,
        ),
        (InternalOperation::GetParam, Operation::GetParam),
        (InternalOperation::SetParam, Operation::SetParam),
        (InternalOperation::Execute, Operation::Execute),
        (InternalOperation::FileAccessRead, Operation::FileAccessRead),
        (
            InternalOperation::FileAccessWrite,
            Operation::FileAccessWrite,
        ),
        (
            InternalOperation::GetFileAccessProgress,
            Operation::GetFileAccessProgress,
        ),
        (
            InternalOperation::MapDepthToPointCloud,
            Operation::MapDepthToPointCloud,
        ),
        (
            InternalOperation::MapDepthToPointCloudRound,
            Operation::MapDepthToPointCloudRound,
        ),
        (InternalOperation::ImageConvert, Operation::ImageConvert),
        (InternalOperation::DepthMosaic, Operation::DepthMosaic),
        (InternalOperation::SaveImage, Operation::SaveImage),
        (InternalOperation::DisplayImage, Operation::DisplayImage),
    ];

    for (internal, public) in cases {
        assert_eq!(map_internal_operation(internal), public);
        assert_eq!(internal.sdk_name(), public.sdk_name());
    }
}

// 验证生命周期与 SDK 错误转换为公开类型，防止内部实现类型泄漏。
#[test]
fn lifecycle_and_sdk_errors_map_without_exposing_internal_types() {
    assert_eq!(
        Error::map_internal_error(InternalError::RuntimeDegraded),
        Error::RuntimeDegraded
    );

    let error = Error::map_internal_error(InternalError::Sdk {
        operation: InternalOperation::GetImage,
        status: 0x8006_0006_u32 as i32,
    });
    assert_eq!(
        error,
        Error::Sdk(SdkError::new(Operation::GetImage, StatusCode::NO_DATA))
    );

    let error = Error::map_internal_error(InternalError::AllocationFailed {
        operation: InternalOperation::GetImage,
        requested: 1024,
    });
    assert_eq!(
        error,
        Error::AllocationFailed {
            operation: Operation::GetImage,
        }
    );
}

// 验证版本错误保留有效文本并拒绝畸形值，防止错误诊断丢失 ABI 信息。
#[test]
fn incompatible_versions_retain_valid_text_and_reject_malformed_text() {
    let error = Error::map_internal_error(InternalError::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: Some(b"1.4.0.0"),
        actual: b"01.4.0.0".to_vec(),
    });
    assert_eq!(
        error,
        Error::IncompatibleSdkVersion {
            minimum: SdkVersion::new(1, 3, 3, 3),
            maximum_exclusive: Some(SdkVersion::new(1, 4, 0, 0)),
            actual: SdkVersion::new(1, 4, 0, 0),
            actual_text: SdkText::new("01.4.0.0").unwrap(),
        }
    );

    let strict = Error::map_internal_error(InternalError::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: None,
        actual: b"1.3.3.4".to_vec(),
    });
    assert_eq!(
        strict,
        Error::IncompatibleSdkVersion {
            minimum: SdkVersion::new(1, 3, 3, 3),
            maximum_exclusive: None,
            actual: SdkVersion::new(1, 3, 3, 4),
            actual_text: SdkText::new("1.3.3.4").unwrap(),
        }
    );
    assert!(strict.to_string().contains("expected exactly 1.3.3.3"));

    let malformed = Error::map_internal_error(InternalError::IncompatibleSdkVersion {
        minimum: b"1.3.3.3",
        maximum_exclusive: None,
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

// 验证状态错误携带正确操作与期望状态，防止调用方收到误导性诊断。
#[test]
fn invalid_states_map_the_operation_and_expected_state() {
    let cases = [
        (
            InternalOperation::RegisterImageDataCallback,
            Operation::RegisterImageDataCallback,
            "open",
        ),
        (
            InternalOperation::RegisterExceptionCallback,
            Operation::RegisterExceptionCallback,
            "open",
        ),
        (
            InternalOperation::GetImage,
            Operation::GetImage,
            "measuring",
        ),
        (
            InternalOperation::GetFileAccessProgress,
            Operation::GetFileAccessProgress,
            "transferring",
        ),
        (
            InternalOperation::GetParam,
            Operation::GetParam,
            "open or measuring",
        ),
    ];

    for (internal, public, expected) in cases {
        assert_eq!(
            Error::map_internal_error(InternalError::InvalidState {
                operation: internal,
                state: "faulted",
            }),
            Error::InvalidState {
                operation: public,
                expected,
                actual: "faulted",
            }
        );
    }
}

// 验证图像契约错误保留字段和长度上下文，防止 FFI 数据错误难以定位。
#[test]
fn image_contract_violations_retain_context() {
    let cases = [
        (
            InternalContract::NullPointerWithLength {
                field: "image data",
                length: 8,
            },
            ContractViolation::NullPointerWithLength {
                field: "image data",
                length: 8,
            },
        ),
        (
            InternalContract::LengthMismatch {
                field: "image data",
                expected: 24,
                actual: 12,
            },
            ContractViolation::LengthMismatch {
                field: "image data",
                expected: 24,
                actual: 12,
            },
        ),
        (
            InternalContract::OutputTooLarge {
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
            InternalContract::InvalidImageValue {
                field: "valid flag",
            },
            ContractViolation::InvalidValue {
                field: "valid flag",
            },
        ),
        (
            InternalContract::NegativeFileProgress {
                completed: -1,
                total: 10,
            },
            ContractViolation::NegativeFileProgress {
                completed: -1,
                total: 10,
            },
        ),
        (
            InternalContract::FileProgressExceedsTotal {
                completed: 11,
                total: 10,
            },
            ContractViolation::FileProgressExceedsTotal {
                completed: 11,
                total: 10,
            },
        ),
    ];

    for (kind, violation) in cases {
        assert_eq!(
            Error::map_internal_error(InternalError::ContractViolation {
                operation: InternalOperation::GetImage,
                kind,
            }),
            Error::ContractViolation {
                operation: Operation::GetImage,
                violation,
            }
        );
    }
}

// 验证图像输入错误映射为公开 violation，防止底层校验细节泄漏。
#[test]
fn invalid_image_inputs_map_to_public_violations() {
    let cases = [
        (
            InternalOperation::MapDepthToPointCloudRound,
            Operation::MapDepthToPointCloudRound,
            InternalInput::ImageCount {
                minimum: 1,
                maximum: 8,
                actual: 0,
            },
            InputViolation::ImageCount {
                minimum: 1,
                maximum: 8,
                actual: 0,
            },
        ),
        (
            InternalOperation::MapDepthToPointCloud,
            Operation::MapDepthToPointCloud,
            InternalInput::UnexpectedImageType {
                expected: 0x0110_00B8,
                actual: 0x0108_0001,
            },
            InputViolation::UnexpectedImageType {
                expected: 0x0110_00B8,
                actual: 0x0108_0001,
            },
        ),
        (
            InternalOperation::ImageConvert,
            Operation::ImageConvert,
            InternalInput::UnsupportedImageConversion {
                source: 0x0108_0001,
                target: 0x0218_0014,
            },
            InputViolation::UnsupportedImageConversion {
                source: 0x0108_0001,
                target: 0x0218_0014,
            },
        ),
        (
            InternalOperation::SaveImage,
            Operation::SaveImage,
            InternalInput::UnsupportedImageFileFormat {
                image_type: 0x0108_0001,
                file_format: 1,
            },
            InputViolation::UnsupportedImageFileFormat {
                image_type: 0x0108_0001,
                file_format: 1,
            },
        ),
        (
            InternalOperation::ImageConvert,
            Operation::ImageConvert,
            InternalInput::InvalidImageLayout {
                field: "data length",
            },
            InputViolation::InvalidImageLayout {
                field: "data length",
            },
        ),
        (
            InternalOperation::GetImage,
            Operation::GetImage,
            InternalInput::TimeoutTooLong {
                maximum_millis: u32::MAX - 1,
                actual_millis: u128::from(u32::MAX),
            },
            InputViolation::TimeoutTooLong {
                maximum_millis: u32::MAX - 1,
                actual_millis: u128::from(u32::MAX),
            },
        ),
        (
            InternalOperation::DepthMosaic,
            Operation::DepthMosaic,
            InternalInput::TooLong {
                maximum: 512,
                actual: 513,
            },
            InputViolation::TooLong {
                max: 512,
                actual: 513,
            },
        ),
    ];

    for (internal_operation, public_operation, kind, violation) in cases {
        assert_eq!(
            Error::map_internal_error(InternalError::InvalidInput {
                operation: internal_operation,
                kind,
            }),
            Error::InvalidInput {
                field: public_operation.sdk_name(),
                violation,
            }
        );
    }
}

// 验证显示输入错误归属公开 DisplayImage 操作，防止 feature 分支改变错误上下文。
#[test]
fn display_input_errors_use_the_public_display_operation() {
    let cases = [
        (
            InternalInput::UnsupportedDisplayImageType {
                actual: 0x0218_0001,
            },
            InputViolation::UnsupportedDisplayImageType {
                actual: 0x0218_0001,
            },
        ),
        (
            InternalInput::UnsupportedDisplayMode {
                image_type: 0x0108_0001,
            },
            InputViolation::UnsupportedDisplayMode {
                image_type: 0x0108_0001,
            },
        ),
        (
            InternalInput::InvalidDisplayRange {
                minimum: 10,
                maximum: 10,
            },
            InputViolation::InvalidDisplayRange {
                minimum: 10,
                maximum: 10,
            },
        ),
    ];

    for (kind, violation) in cases {
        assert_eq!(
            Error::map_internal_error(InternalError::InvalidInput {
                operation: InternalOperation::DisplayImage,
                kind,
            }),
            Error::InvalidInput {
                field: Operation::DisplayImage.sdk_name(),
                violation,
            }
        );
    }
}

// 验证组合清理错误递归保留 source，防止 stop、close 与 open 根因丢失。
#[test]
fn cleanup_and_open_failure_sources_map_recursively() {
    let error = Error::map_device_cleanup_error(mv3d_lp_internal::DeviceCleanupError {
        stop: Some(Box::new(InternalError::OpenFailedWithHandle {
            operation: InternalOperation::OpenDeviceByIp,
            source: Box::new(InternalError::Sdk {
                operation: InternalOperation::OpenDeviceByIp,
                status: 0x8006_0004_u32 as i32,
            }),
        })),
        close: Some(Box::new(InternalError::Sdk {
            operation: InternalOperation::CloseDevice,
            status: 0x8006_0000_u32 as i32,
        })),
    });

    assert_eq!(
        error,
        Error::DeviceCleanup {
            stop: Some(Box::new(Error::OpenFailedWithHandle {
                operation: Operation::OpenDeviceByIp,
                source: Box::new(Error::Sdk(SdkError::new(
                    Operation::OpenDeviceByIp,
                    StatusCode::INVALID_PARAMETER,
                ))),
            })),
            close: Some(Box::new(Error::Sdk(SdkError::new(
                Operation::CloseDevice,
                StatusCode::INVALID_HANDLE,
            )))),
        }
    );

    let stop = error
        .source()
        .expect("cleanup should expose its stop error");
    let source = stop
        .source()
        .expect("open failure should expose its nested SDK error");
    assert!(source.to_string().contains("MV3D_LP_E_PARAMETER"));
}
