use crate::{SdkText, SdkVersion};
use std::error::Error as StdError;
use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

/// Identifies the SDK operation associated with an error.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Operation {
    GetVersion,
    Initialize,
    Finalize,
    GetDeviceNumber,
    GetDeviceList,
    OpenDeviceByIp,
    OpenDeviceBySn,
    CloseDevice,
    SetIpConfig,
    StartMeasure,
    StopMeasure,
    SoftTrigger,
    ClearDataBuffer,
    GetImage,
    RegisterImageDataCallback,
    RegisterExceptionCallback,
    GetParam,
    SetParam,
    Execute,
    FileAccessRead,
    FileAccessWrite,
    GetFileAccessProgress,
    MapDepthToPointCloud,
    MapDepthToPointCloudRound,
    ImageConvert,
    DepthMosaic,
    SaveImage,
    #[cfg(feature = "display-windows")]
    DisplayImage,
}

impl Operation {
    #[must_use]
    pub const fn sdk_name(self) -> &'static str {
        match self {
            Self::GetVersion => "MV3D_LP_GetVersion",
            Self::Initialize => "MV3D_LP_Initialize",
            Self::Finalize => "MV3D_LP_Finalize",
            Self::GetDeviceNumber => "MV3D_LP_GetDeviceNumber",
            Self::GetDeviceList => "MV3D_LP_GetDeviceList",
            Self::OpenDeviceByIp => "MV3D_LP_OpenDeviceByIP",
            Self::OpenDeviceBySn => "MV3D_LP_OpenDeviceBySN",
            Self::CloseDevice => "MV3D_LP_CloseDevice",
            Self::SetIpConfig => "MV3D_LP_SetIpConfig",
            Self::StartMeasure => "MV3D_LP_StartMeasure",
            Self::StopMeasure => "MV3D_LP_StopMeasure",
            Self::SoftTrigger => "MV3D_LP_SoftTrigger",
            Self::ClearDataBuffer => "MV3D_LP_ClearDataBuffer",
            Self::GetImage => "MV3D_LP_GetImage",
            Self::RegisterImageDataCallback => "MV3D_LP_RegisterImageDataCallBack",
            Self::RegisterExceptionCallback => "MV3D_LP_RegisterExceptionCallBack",
            Self::GetParam => "MV3D_LP_GetParam",
            Self::SetParam => "MV3D_LP_SetParam",
            Self::Execute => "MV3D_LP_Execute",
            Self::FileAccessRead => "MV3D_LP_FileAccessRead",
            Self::FileAccessWrite => "MV3D_LP_FileAccessWrite",
            Self::GetFileAccessProgress => "MV3D_LP_GetFileAccessProgress",
            Self::MapDepthToPointCloud => "MV3D_LP_MapDepthToPointCloud",
            Self::MapDepthToPointCloudRound => "MV3D_LP_MapDepthToPointCloudRound",
            Self::ImageConvert => "MV3D_LP_ImageConvert",
            Self::DepthMosaic => "MV3D_LP_DepthMosaic",
            Self::SaveImage => "MV3D_LP_SaveImage",
            #[cfg(feature = "display-windows")]
            Self::DisplayImage => "MV3D_LP_DisplayImage",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.sdk_name())
    }
}

/// A status returned by the SDK, stored as its exact 32-bit bit pattern.
///
/// This is deliberately a newtype rather than a Rust enum so that statuses
/// introduced by a newer runtime remain representable.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct StatusCode(u32);

impl StatusCode {
    pub const OK: Self = Self(0x0000_0000);
    pub const INVALID_HANDLE: Self = Self(0x8006_0000);
    pub const UNSUPPORTED: Self = Self(0x8006_0001);
    pub const BUFFER_OVERFLOW: Self = Self(0x8006_0002);
    pub const INVALID_CALL_ORDER: Self = Self(0x8006_0003);
    pub const INVALID_PARAMETER: Self = Self(0x8006_0004);
    pub const RESOURCE_ERROR: Self = Self(0x8006_0005);
    pub const NO_DATA: Self = Self(0x8006_0006);
    pub const PRECONDITION_FAILED: Self = Self(0x8006_0007);
    pub const VERSION_MISMATCH: Self = Self(0x8006_0008);
    pub const INSUFFICIENT_BUFFER: Self = Self(0x8006_0009);
    pub const ABNORMAL_IMAGE: Self = Self(0x8006_000A);
    pub const LOAD_LIBRARY_FAILED: Self = Self(0x8006_000B);
    pub const ALGORITHM_ERROR: Self = Self(0x8006_000C);
    pub const DEVICE_OFFLINE: Self = Self(0x8006_000D);
    pub const ACCESS_DENIED: Self = Self(0x8006_000E);
    pub const OUT_OF_RANGE: Self = Self(0x8006_000F);
    pub const UNKNOWN: Self = Self(0x8006_00FF);

    #[must_use]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw as u32)
    }

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0 as i32
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn is_ok(self) -> bool {
        self.0 == Self::OK.0
    }

    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0x0000_0000 => Some("MV3D_LP_OK"),
            0x8006_0000 => Some("MV3D_LP_E_HANDLE"),
            0x8006_0001 => Some("MV3D_LP_E_SUPPORT"),
            0x8006_0002 => Some("MV3D_LP_E_BUFOVER"),
            0x8006_0003 => Some("MV3D_LP_E_CALLORDER"),
            0x8006_0004 => Some("MV3D_LP_E_PARAMETER"),
            0x8006_0005 => Some("MV3D_LP_E_RESOURCE"),
            0x8006_0006 => Some("MV3D_LP_E_NODATA"),
            0x8006_0007 => Some("MV3D_LP_E_PRECONDITION"),
            0x8006_0008 => Some("MV3D_LP_E_VERSION"),
            0x8006_0009 => Some("MV3D_LP_E_NOENOUGH_BUF"),
            0x8006_000A => Some("MV3D_LP_E_ABNORMAL_IMAGE"),
            0x8006_000B => Some("MV3D_LP_E_LOAD_LIBRARY"),
            0x8006_000C => Some("MV3D_LP_E_ALGORITHM"),
            0x8006_000D => Some("MV3D_LP_E_DEVICE_OFFLINE"),
            0x8006_000E => Some("MV3D_LP_E_ACCESS_DENIED"),
            0x8006_000F => Some("MV3D_LP_E_OUTOFRANGE"),
            0x8006_00FF => Some("MV3D_LP_E_UNKNOW"),
            _ => None,
        }
    }
}

impl fmt::Debug for StatusCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "StatusCode({name}, 0x{:08X})", self.0),
            None => write!(formatter, "StatusCode(0x{:08X})", self.0),
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "{name} (0x{:08X})", self.0),
            None => write!(formatter, "unknown SDK status 0x{:08X}", self.0),
        }
    }
}

/// An error reported directly by an SDK function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SdkError {
    operation: Operation,
    status: StatusCode,
}

impl SdkError {
    #[must_use]
    pub const fn new(operation: Operation, status: StatusCode) -> Self {
        Self { operation, status }
    }

    #[must_use]
    pub const fn operation(self) -> Operation {
        self.operation
    }

    #[must_use]
    pub const fn status(self) -> StatusCode {
        self.status
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed with {}", self.operation, self.status)
    }
}

impl StdError for SdkError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InputViolation {
    Empty,
    NonAscii,
    InteriorNul,
    TooLong {
        max: usize,
        actual: usize,
    },
    CallbackQueueCapacity {
        maximum: usize,
        actual: usize,
    },
    TimeoutTooLong {
        maximum_millis: u32,
        actual_millis: u128,
    },
    ImageCount {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    UnexpectedImageType {
        expected: u32,
        actual: u32,
    },
    UnsupportedImageConversion {
        source: u32,
        target: u32,
    },
    UnsupportedImageFileFormat {
        image_type: u32,
        file_format: i32,
    },
    InvalidImageLayout {
        field: &'static str,
    },
    UnsupportedDisplayImageType {
        actual: u32,
    },
    UnsupportedDisplayMode {
        image_type: u32,
    },
    InvalidDisplayRange {
        minimum: i32,
        maximum: i32,
    },
    WindowHandleUnavailable,
    WindowHandleNotSupported,
    NonWin32Window,
}

impl fmt::Display for InputViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("the value is empty"),
            Self::NonAscii => formatter.write_str("the value is not ASCII"),
            Self::InteriorNul => formatter.write_str("the value contains a NUL byte"),
            Self::TooLong { max, actual } => {
                write!(
                    formatter,
                    "the value has {actual} bytes; at most {max} are allowed"
                )
            }
            Self::CallbackQueueCapacity { maximum, actual } => write!(
                formatter,
                "callback queue capacity is {actual}; at most {maximum} events are allowed"
            ),
            Self::TimeoutTooLong {
                maximum_millis,
                actual_millis,
            } => write!(
                formatter,
                "the timeout is {actual_millis} milliseconds; at most {maximum_millis} milliseconds are allowed"
            ),
            Self::ImageCount {
                minimum,
                maximum,
                actual,
            } => write!(
                formatter,
                "the image count is {actual}; expected {minimum}..={maximum}"
            ),
            Self::UnexpectedImageType { expected, actual } => write!(
                formatter,
                "image type 0x{actual:08X} does not match required type 0x{expected:08X}"
            ),
            Self::UnsupportedImageConversion { source, target } => write!(
                formatter,
                "conversion from image type 0x{source:08X} to 0x{target:08X} is unsupported"
            ),
            Self::UnsupportedImageFileFormat {
                image_type,
                file_format,
            } => write!(
                formatter,
                "image type 0x{image_type:08X} cannot be saved as file format {file_format}"
            ),
            Self::InvalidImageLayout { field } => {
                write!(formatter, "the image has an invalid {field}")
            }
            Self::UnsupportedDisplayImageType { actual } => write!(
                formatter,
                "image type 0x{actual:08X} cannot be displayed by the SDK"
            ),
            Self::UnsupportedDisplayMode { image_type } => write!(
                formatter,
                "the requested display mode is unsupported for image type 0x{image_type:08X}"
            ),
            Self::InvalidDisplayRange { minimum, maximum } => write!(
                formatter,
                "manual display range requires minimum < maximum, got {minimum}..{maximum}"
            ),
            Self::WindowHandleUnavailable => {
                formatter.write_str("the window handle is unavailable")
            }
            Self::WindowHandleNotSupported => {
                formatter.write_str("the window handle cannot be represented")
            }
            Self::NonWin32Window => formatter.write_str("the window does not expose a Win32 HWND"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContractViolation {
    NullPointer {
        field: &'static str,
    },
    NullPointerWithLength {
        field: &'static str,
        length: usize,
    },
    CountExceedsCapacity {
        field: &'static str,
        count: usize,
        capacity: usize,
    },
    MissingNul {
        field: &'static str,
        capacity: usize,
    },
    UnknownDiscriminant {
        field: &'static str,
        raw: u32,
    },
    LengthOverflow {
        field: &'static str,
    },
    LengthMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    OutputTooLarge {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    InvalidValue {
        field: &'static str,
    },
    NegativeFileProgress {
        completed: i64,
        total: i64,
    },
    FileProgressExceedsTotal {
        completed: u64,
        total: u64,
    },
    FileProgressRegressed {
        previous: u64,
        current: u64,
    },
    FileProgressTotalChanged {
        previous: u64,
        current: u64,
    },
}

impl fmt::Display for ContractViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullPointer { field } => write!(formatter, "{field} is null"),
            Self::NullPointerWithLength { field, length } => {
                write!(formatter, "{field} is null but its length is {length}")
            }
            Self::CountExceedsCapacity {
                field,
                count,
                capacity,
            } => write!(
                formatter,
                "{field} count {count} exceeds capacity {capacity}"
            ),
            Self::MissingNul { field, capacity } => write!(
                formatter,
                "{field} has no NUL terminator within its {capacity}-byte field"
            ),
            Self::UnknownDiscriminant { field, raw } => {
                write!(formatter, "{field} contains unknown value 0x{raw:08X}")
            }
            Self::LengthOverflow { field } => {
                write!(formatter, "the computed length for {field} overflowed")
            }
            Self::LengthMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} has length {actual}; the SDK image metadata requires at least {expected}"
            ),
            Self::OutputTooLarge {
                field,
                limit,
                actual,
            } => write!(
                formatter,
                "{field} has {actual} bytes, exceeding the configured limit of {limit}"
            ),
            Self::InvalidValue { field } => {
                write!(formatter, "{field} contains an invalid SDK value")
            }
            Self::NegativeFileProgress { completed, total } => write!(
                formatter,
                "file progress contains negative values: completed={completed}, total={total}"
            ),
            Self::FileProgressExceedsTotal { completed, total } => {
                write!(formatter, "file progress {completed} exceeds total {total}")
            }
            Self::FileProgressRegressed { previous, current } => write!(
                formatter,
                "file progress regressed from {previous} to {current}"
            ),
            Self::FileProgressTotalChanged { previous, current } => write!(
                formatter,
                "file progress total changed from {previous} to {current}"
            ),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    UnsupportedPlatform,
    Sdk(SdkError),
    InvalidInput {
        field: &'static str,
        violation: InputViolation,
    },
    InvalidState {
        operation: Operation,
        expected: &'static str,
        actual: &'static str,
    },
    ContractViolation {
        operation: Operation,
        violation: ContractViolation,
    },
    OpenFailedWithHandle {
        operation: Operation,
        source: Box<Error>,
    },
    DiscoveryChanged {
        attempts: usize,
    },
    IncompatibleSdkVersion {
        expected: SdkVersion,
        actual: SdkVersion,
        actual_text: SdkText,
    },
    RuntimeAlreadyActive,
    RuntimeTerminal,
    AllocationFailed {
        operation: Operation,
    },
    CallbackWorkerSpawn,
    DeviceCleanup {
        stop: Option<Box<Error>>,
        close: Option<Box<Error>>,
    },
    UnclosedDevices {
        live_handles: usize,
        teardown_uncertain: bool,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter
                .write_str("native 3DMVS support is available only on x86_64-pc-windows-msvc"),
            Self::Sdk(error) => error.fmt(formatter),
            Self::InvalidInput { field, violation } => {
                write!(formatter, "invalid {field}: {violation}")
            }
            Self::InvalidState {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "{operation} requires state {expected}, but the device is {actual}"
            ),
            Self::ContractViolation {
                operation,
                violation,
            } => write!(
                formatter,
                "{operation} returned data that violates the SDK contract: {violation}"
            ),
            Self::OpenFailedWithHandle { operation, source } => write!(
                formatter,
                "{operation} failed and returned a non-null handle that cannot be used safely: {source}"
            ),
            Self::DiscoveryChanged { attempts } => write!(
                formatter,
                "the device list kept changing during {attempts} discovery attempts"
            ),
            Self::IncompatibleSdkVersion {
                expected,
                actual_text,
                ..
            } => write!(
                formatter,
                "incompatible SDK runtime version {actual_text}; expected {expected}"
            ),
            Self::RuntimeAlreadyActive => {
                formatter.write_str("the process already has an active 3DMVS SDK runtime")
            }
            Self::RuntimeTerminal => formatter.write_str(
                "the process-wide 3DMVS SDK runtime is terminal and cannot be initialized again",
            ),
            Self::AllocationFailed { operation } => {
                write!(
                    formatter,
                    "memory allocation failed while preparing {operation}"
                )
            }
            Self::CallbackWorkerSpawn => {
                formatter.write_str("could not spawn the Rust callback worker thread")
            }
            Self::DeviceCleanup { stop, close } => match (stop, close) {
                (Some(stop), Some(close)) => write!(
                    formatter,
                    "device cleanup failed while stopping ({stop}) and closing ({close})"
                ),
                (Some(stop), None) => write!(formatter, "device cleanup failed: {stop}"),
                (None, Some(close)) => write!(formatter, "device cleanup failed: {close}"),
                (None, None) => formatter.write_str("device cleanup failed without an SDK status"),
            },
            Self::UnclosedDevices {
                live_handles,
                teardown_uncertain,
            } => write!(
                formatter,
                "SDK finalization was skipped because {live_handles} handle(s) remain live and teardown uncertainty is {teardown_uncertain}"
            ),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Sdk(error) => Some(error),
            Self::OpenFailedWithHandle { source, .. } => Some(source.as_ref()),
            Self::DeviceCleanup {
                stop: Some(error), ..
            }
            | Self::DeviceCleanup {
                stop: None,
                close: Some(error),
            } => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<SdkError> for Error {
    fn from(error: SdkError) -> Self {
        Self::Sdk(error)
    }
}

impl From<mv3d_lp_internal::DeviceCleanupError> for Error {
    fn from(error: mv3d_lp_internal::DeviceCleanupError) -> Self {
        Self::DeviceCleanup {
            stop: error.stop.map(|error| Box::new(Self::from(*error))),
            close: error.close.map(|error| Box::new(Self::from(*error))),
        }
    }
}

impl From<mv3d_lp_internal::Error> for Error {
    fn from(error: mv3d_lp_internal::Error) -> Self {
        use mv3d_lp_internal::{ContractViolation as InternalContract, Error as InternalError};

        match error {
            InternalError::UnsupportedPlatform => Self::UnsupportedPlatform,
            InternalError::RuntimeAlreadyActive => Self::RuntimeAlreadyActive,
            InternalError::RuntimeTerminal => Self::RuntimeTerminal,
            InternalError::IncompatibleSdkVersion { expected, actual } => {
                let expected = parse_version_bytes(expected);
                let actual = SdkText::try_from(actual).ok().and_then(|actual_text| {
                    parse_version_bytes_checked(actual_text.as_bytes())
                        .map(|actual| (actual, actual_text))
                });
                match actual {
                    Some((actual, actual_text)) => Self::IncompatibleSdkVersion {
                        expected,
                        actual,
                        actual_text,
                    },
                    None => Self::ContractViolation {
                        operation: Operation::GetVersion,
                        violation: ContractViolation::InvalidValue {
                            field: "SDK version",
                        },
                    },
                }
            }
            InternalError::Sdk { operation, status } => Self::Sdk(SdkError::new(
                operation_from_sdk_name(operation),
                StatusCode::from_raw(status),
            )),
            InternalError::InvalidState { operation, state } => {
                let operation = operation_from_sdk_name(operation);
                let expected = match operation {
                    Operation::RegisterImageDataCallback | Operation::RegisterExceptionCallback => {
                        "open with no previous callback registration"
                    }
                    Operation::StartMeasure
                    | Operation::FileAccessRead
                    | Operation::FileAccessWrite => "open",
                    Operation::StopMeasure | Operation::SoftTrigger | Operation::GetImage => {
                        "measuring"
                    }
                    Operation::GetFileAccessProgress => "transferring",
                    _ => "open or measuring",
                };
                Self::InvalidState {
                    operation,
                    expected,
                    actual: state,
                }
            }
            InternalError::InvalidInput { operation, kind } => Self::InvalidInput {
                field: operation,
                violation: match kind {
                    mv3d_lp_internal::InvalidInput::Empty => InputViolation::Empty,
                    mv3d_lp_internal::InvalidInput::InteriorNul => InputViolation::InteriorNul,
                    mv3d_lp_internal::InvalidInput::NonAscii => InputViolation::NonAscii,
                    mv3d_lp_internal::InvalidInput::TooLong { actual, maximum } => {
                        InputViolation::TooLong {
                            max: maximum,
                            actual,
                        }
                    }
                    mv3d_lp_internal::InvalidInput::TimeoutTooLong {
                        maximum_millis,
                        actual_millis,
                    } => InputViolation::TimeoutTooLong {
                        maximum_millis,
                        actual_millis,
                    },
                    mv3d_lp_internal::InvalidInput::ImageCount {
                        minimum,
                        maximum,
                        actual,
                    } => InputViolation::ImageCount {
                        minimum,
                        maximum,
                        actual,
                    },
                    mv3d_lp_internal::InvalidInput::UnexpectedImageType { expected, actual } => {
                        InputViolation::UnexpectedImageType { expected, actual }
                    }
                    mv3d_lp_internal::InvalidInput::UnsupportedImageConversion {
                        source,
                        target,
                    } => InputViolation::UnsupportedImageConversion { source, target },
                    mv3d_lp_internal::InvalidInput::UnsupportedImageFileFormat {
                        image_type,
                        file_format,
                    } => InputViolation::UnsupportedImageFileFormat {
                        image_type,
                        file_format,
                    },
                    mv3d_lp_internal::InvalidInput::InvalidImageLayout { field } => {
                        InputViolation::InvalidImageLayout { field }
                    }
                },
            },
            InternalError::ContractViolation { operation, kind } => {
                let operation = operation_from_sdk_name(operation);
                let violation = match kind {
                    InternalContract::NullVersionPointer => ContractViolation::NullPointer {
                        field: "SDK version",
                    },
                    InternalContract::UnterminatedVersion { limit } => {
                        ContractViolation::MissingNul {
                            field: "SDK version",
                            capacity: limit,
                        }
                    }
                    InternalContract::NullHandleOnSuccess => ContractViolation::NullPointer {
                        field: "device handle",
                    },
                    InternalContract::DeviceCountExceedsLimit { reported, limit } => {
                        ContractViolation::OutputTooLarge {
                            field: "device count",
                            limit,
                            actual: usize::try_from(reported).unwrap_or(usize::MAX),
                        }
                    }
                    InternalContract::DeviceListCountMismatch { reported, returned } => {
                        ContractViolation::CountExceedsCapacity {
                            field: "device list",
                            count: usize::try_from(reported).unwrap_or(usize::MAX),
                            capacity: returned,
                        }
                    }
                    InternalContract::UnknownParameterType(raw) => {
                        ContractViolation::UnknownDiscriminant {
                            field: "parameter type",
                            raw: raw as u32,
                        }
                    }
                    InternalContract::EnumCountExceedsLimit { reported, limit } => {
                        ContractViolation::CountExceedsCapacity {
                            field: "supported enumeration values",
                            count: usize::try_from(reported).unwrap_or(usize::MAX),
                            capacity: limit,
                        }
                    }
                    InternalContract::StringMaxLengthExceedsLimit { reported, limit } => {
                        ContractViolation::OutputTooLarge {
                            field: "parameter string maximum length",
                            limit,
                            actual: usize::try_from(reported).unwrap_or(usize::MAX),
                        }
                    }
                    InternalContract::HandleCountOverflow => ContractViolation::LengthOverflow {
                        field: "live device handle count",
                    },
                    InternalContract::CallbackCookieExhausted => {
                        ContractViolation::LengthOverflow {
                            field: "callback cookie sequence",
                        }
                    }
                    InternalContract::NullPointerWithLength { field, length } => {
                        ContractViolation::NullPointerWithLength { field, length }
                    }
                    InternalContract::LengthMismatch {
                        field,
                        expected,
                        actual,
                    } => ContractViolation::LengthMismatch {
                        field,
                        expected,
                        actual,
                    },
                    InternalContract::LengthOverflow { field } => {
                        ContractViolation::LengthOverflow { field }
                    }
                    InternalContract::OutputTooLarge {
                        field,
                        limit,
                        actual,
                    } => ContractViolation::OutputTooLarge {
                        field,
                        limit,
                        actual,
                    },
                    InternalContract::InvalidImageValue { field } => {
                        ContractViolation::InvalidValue { field }
                    }
                    InternalContract::NegativeFileProgress { completed, total } => {
                        ContractViolation::NegativeFileProgress { completed, total }
                    }
                    InternalContract::FileProgressExceedsTotal { completed, total } => {
                        ContractViolation::FileProgressExceedsTotal { completed, total }
                    }
                    InternalContract::FileProgressRegressed { previous, current } => {
                        ContractViolation::FileProgressRegressed { previous, current }
                    }
                    InternalContract::FileProgressTotalChanged { previous, current } => {
                        ContractViolation::FileProgressTotalChanged { previous, current }
                    }
                };
                Self::ContractViolation {
                    operation,
                    violation,
                }
            }
            InternalError::OpenFailedWithHandle { operation, source } => {
                Self::OpenFailedWithHandle {
                    operation: operation_from_sdk_name(operation),
                    source: Box::new(Self::from(*source)),
                }
            }
            InternalError::DiscoveryChanged { attempts } => Self::DiscoveryChanged { attempts },
            InternalError::AllocationFailed { operation, .. } => Self::AllocationFailed {
                operation: operation_from_sdk_name(operation),
            },
            InternalError::UnclosedDevices {
                live_handles,
                teardown_uncertain,
            } => Self::UnclosedDevices {
                live_handles,
                teardown_uncertain,
            },
        }
    }
}

pub(crate) fn operation_from_sdk_name(name: &'static str) -> Operation {
    match name {
        "MV3D_LP_GetVersion" => Operation::GetVersion,
        "MV3D_LP_Initialize" => Operation::Initialize,
        "MV3D_LP_Finalize" => Operation::Finalize,
        "MV3D_LP_GetDeviceNumber" => Operation::GetDeviceNumber,
        "MV3D_LP_GetDeviceList" => Operation::GetDeviceList,
        "MV3D_LP_OpenDeviceByIP" => Operation::OpenDeviceByIp,
        "MV3D_LP_OpenDeviceBySN" => Operation::OpenDeviceBySn,
        "MV3D_LP_CloseDevice" => Operation::CloseDevice,
        "MV3D_LP_SetIpConfig" => Operation::SetIpConfig,
        "MV3D_LP_StartMeasure" => Operation::StartMeasure,
        "MV3D_LP_StopMeasure" => Operation::StopMeasure,
        "MV3D_LP_SoftTrigger" => Operation::SoftTrigger,
        "MV3D_LP_ClearDataBuffer" => Operation::ClearDataBuffer,
        "MV3D_LP_GetImage" => Operation::GetImage,
        "MV3D_LP_RegisterImageDataCallBack" => Operation::RegisterImageDataCallback,
        "MV3D_LP_RegisterExceptionCallBack" => Operation::RegisterExceptionCallback,
        "MV3D_LP_GetParam" => Operation::GetParam,
        "MV3D_LP_SetParam" => Operation::SetParam,
        "MV3D_LP_Execute" => Operation::Execute,
        "MV3D_LP_FileAccessRead" => Operation::FileAccessRead,
        "MV3D_LP_FileAccessWrite" => Operation::FileAccessWrite,
        "MV3D_LP_GetFileAccessProgress" => Operation::GetFileAccessProgress,
        "MV3D_LP_MapDepthToPointCloud" => Operation::MapDepthToPointCloud,
        "MV3D_LP_MapDepthToPointCloudRound" => Operation::MapDepthToPointCloudRound,
        "MV3D_LP_ImageConvert" => Operation::ImageConvert,
        "MV3D_LP_DepthMosaic" => Operation::DepthMosaic,
        "MV3D_LP_SaveImage" => Operation::SaveImage,
        #[cfg(feature = "display-windows")]
        "MV3D_LP_DisplayImage" => Operation::DisplayImage,
        _ => unreachable!("internal wrapper used an unknown operation name: {name}"),
    }
}

fn parse_version_bytes(bytes: &[u8]) -> SdkVersion {
    parse_version_bytes_checked(bytes).expect("the audited expected version is valid")
}

fn parse_version_bytes_checked(bytes: &[u8]) -> Option<SdkVersion> {
    std::str::from_utf8(bytes).ok()?.parse().ok()
}
