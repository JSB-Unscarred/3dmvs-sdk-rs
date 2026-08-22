use std::error::Error as StdError;
use std::fmt;

use crate::bindings;

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

macro_rules! status_codes {
    ($($constant:ident = $raw:expr => $name:literal),+ $(,)?) => {
        impl StatusCode {
            $(pub const $constant: Self = Self($raw as u32);)+

            /// Returns the vendor header name; statuses from a newer runtime return `None`.
            #[must_use]
            pub const fn name(self) -> Option<&'static str> {
                $(if self.0 == Self::$constant.0 {
                    return Some($name);
                })+
                None
            }
        }
    };
}

// 位模式与名字都以 bindings 为唯一来源，厂商头文件升级时只改 bindings。
status_codes! {
    OK = 0 => "MV3D_LP_OK",
    INVALID_HANDLE = bindings::MV3D_LP_E_HANDLE => "MV3D_LP_E_HANDLE",
    UNSUPPORTED = bindings::MV3D_LP_E_SUPPORT => "MV3D_LP_E_SUPPORT",
    BUFFER_OVERFLOW = bindings::MV3D_LP_E_BUFOVER => "MV3D_LP_E_BUFOVER",
    INVALID_CALL_ORDER = bindings::MV3D_LP_E_CALLORDER => "MV3D_LP_E_CALLORDER",
    INVALID_PARAMETER = bindings::MV3D_LP_E_PARAMETER => "MV3D_LP_E_PARAMETER",
    RESOURCE_ERROR = bindings::MV3D_LP_E_RESOURCE => "MV3D_LP_E_RESOURCE",
    NO_DATA = bindings::MV3D_LP_E_NODATA => "MV3D_LP_E_NODATA",
    PRECONDITION_FAILED = bindings::MV3D_LP_E_PRECONDITION => "MV3D_LP_E_PRECONDITION",
    VERSION_MISMATCH = bindings::MV3D_LP_E_VERSION => "MV3D_LP_E_VERSION",
    INSUFFICIENT_BUFFER = bindings::MV3D_LP_E_NOENOUGH_BUF => "MV3D_LP_E_NOENOUGH_BUF",
    ABNORMAL_IMAGE = bindings::MV3D_LP_E_ABNORMAL_IMAGE => "MV3D_LP_E_ABNORMAL_IMAGE",
    LOAD_LIBRARY_FAILED = bindings::MV3D_LP_E_LOAD_LIBRARY => "MV3D_LP_E_LOAD_LIBRARY",
    ALGORITHM_ERROR = bindings::MV3D_LP_E_ALGORITHM => "MV3D_LP_E_ALGORITHM",
    DEVICE_OFFLINE = bindings::MV3D_LP_E_DEVICE_OFFLINE => "MV3D_LP_E_DEVICE_OFFLINE",
    ACCESS_DENIED = bindings::MV3D_LP_E_ACCESS_DENIED => "MV3D_LP_E_ACCESS_DENIED",
    OUT_OF_RANGE = bindings::MV3D_LP_E_OUTOFRANGE => "MV3D_LP_E_OUTOFRANGE",
    UNKNOWN = bindings::MV3D_LP_E_UNKNOW => "MV3D_LP_E_UNKNOW",
}

impl StatusCode {
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
    InteriorNul,
    TooLong {
        max: usize,
        actual: usize,
    },
    ImageCount {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    InvalidImageLayout {
        field: &'static str,
    },
    WindowHandleUnavailable,
    WindowHandleNotSupported,
    NonWin32Window,
}

impl fmt::Display for InputViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("the value is empty"),
            Self::InteriorNul => formatter.write_str("the value contains a NUL byte"),
            Self::TooLong { max, actual } => write!(
                formatter,
                "the value has {actual} bytes; at most {max} are allowed"
            ),
            Self::ImageCount {
                minimum,
                maximum,
                actual,
            } => write!(
                formatter,
                "the image count is {actual}; expected {minimum}..={maximum}"
            ),
            Self::InvalidImageLayout { field } => {
                write!(formatter, "the image has an invalid {field}")
            }
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
                "{field} has length {actual}; expected {expected} for this SDK result"
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
    DeviceCleanup {
        stop: Box<Error>,
        close: Box<Error>,
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
                "{operation} requires state {expected}, but the current state is {actual}"
            ),
            Self::ContractViolation {
                operation,
                violation,
            } => write!(
                formatter,
                "{operation} returned data that violates the SDK contract: {violation}"
            ),
            Self::DeviceCleanup { stop, close } => write!(
                formatter,
                "device cleanup failed while stopping ({stop}) and closing ({close})"
            ),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Sdk(error) => Some(error),
            Self::DeviceCleanup { close, .. } => Some(close.as_ref()),
            _ => None,
        }
    }
}

impl From<SdkError> for Error {
    fn from(error: SdkError) -> Self {
        Self::Sdk(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{Error, Operation, SdkError, StatusCode};

    // 验证已知与未知 status 都保留厂商位模式和调用上下文。
    #[test]
    fn status_preserves_bits_and_operation() {
        let known = StatusCode::from_raw(0x8006_000D_u32 as i32);
        assert_eq!(known, StatusCode::DEVICE_OFFLINE);
        assert_eq!(known.bits(), 0x8006_000D);

        let unknown = StatusCode::from_bits(0xDEAD_BEEF);
        let error = SdkError::new(Operation::GetParam, unknown);
        assert_eq!(error.operation(), Operation::GetParam);
        assert_eq!(error.status(), unknown);
    }

    // 验证双重清理失败保留两次调用信息，并以 terminal Close 作为 source。
    #[test]
    fn cleanup_preserves_stop_and_close_failures() {
        let stop = SdkError::new(Operation::StopMeasure, StatusCode::RESOURCE_ERROR);
        let close = SdkError::new(Operation::CloseDevice, StatusCode::INVALID_HANDLE);
        let error = Error::DeviceCleanup {
            stop: Box::new(stop.into()),
            close: Box::new(close.into()),
        };

        assert_eq!(
            error.to_string(),
            format!("device cleanup failed while stopping ({stop}) and closing ({close})")
        );
        assert_eq!(
            StdError::source(&error).map(ToString::to_string),
            Some(close.to_string())
        );
    }
}
