use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractViolation {
    NullVersionPointer,
    UnterminatedVersion {
        limit: usize,
    },
    NullHandleOnSuccess,
    DeviceCountExceedsLimit {
        reported: u32,
        limit: usize,
    },
    DeviceListCountMismatch {
        reported: u32,
        returned: usize,
    },
    UnknownParameterType(i32),
    EnumCountExceedsLimit {
        reported: u32,
        limit: usize,
    },
    StringMaxLengthExceedsLimit {
        reported: u32,
        limit: usize,
    },
    HandleCountOverflow,
    CallbackCookieExhausted,
    NullPointerWithLength {
        field: &'static str,
        length: usize,
    },
    LengthMismatch {
        field: &'static str,
        expected: usize,
        actual: usize,
    },
    LengthOverflow {
        field: &'static str,
    },
    OutputTooLarge {
        field: &'static str,
        limit: usize,
        actual: usize,
    },
    InvalidImageValue {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvalidInput {
    Empty,
    InteriorNul,
    NonAscii,
    TooLong {
        actual: usize,
        maximum: usize,
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    UnsupportedPlatform,
    RuntimeAlreadyActive,
    RuntimeTerminal,
    IncompatibleSdkVersion {
        expected: &'static [u8],
        actual: Vec<u8>,
    },
    Sdk {
        operation: &'static str,
        status: i32,
    },
    InvalidState {
        operation: &'static str,
        state: &'static str,
    },
    InvalidInput {
        operation: &'static str,
        kind: InvalidInput,
    },
    ContractViolation {
        operation: &'static str,
        kind: ContractViolation,
    },
    OpenFailedWithHandle {
        operation: &'static str,
        source: Box<Error>,
    },
    DiscoveryChanged {
        attempts: usize,
    },
    AllocationFailed {
        operation: &'static str,
        requested: usize,
    },
    UnclosedDevices {
        live_handles: usize,
        teardown_uncertain: bool,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str(
                "native 3DMVS support requires x86_64-pc-windows-msvc and the `native` feature",
            ),
            Self::RuntimeAlreadyActive => f.write_str("the 3DMVS runtime is already active"),
            Self::RuntimeTerminal => {
                f.write_str("the 3DMVS runtime has reached its terminal state")
            }
            Self::IncompatibleSdkVersion { expected, actual } => write!(
                f,
                "incompatible 3DMVS runtime version: expected {}, got {}",
                String::from_utf8_lossy(expected),
                String::from_utf8_lossy(actual)
            ),
            Self::Sdk { operation, status } => {
                write!(
                    f,
                    "{operation} failed with SDK status 0x{:08X}",
                    *status as u32
                )
            }
            Self::InvalidState { operation, state } => {
                write!(f, "{operation} is not valid while the device is {state}")
            }
            Self::InvalidInput { operation, kind } => {
                write!(f, "invalid input for {operation}: {kind:?}")
            }
            Self::ContractViolation { operation, kind } => {
                write!(f, "the SDK violated the {operation} contract: {kind:?}")
            }
            Self::OpenFailedWithHandle { operation, source } => write!(
                f,
                "{operation} failed and also returned a non-null, unusable handle: {source}"
            ),
            Self::DiscoveryChanged { attempts } => write!(
                f,
                "the device list kept changing across {attempts} discovery attempts"
            ),
            Self::AllocationFailed {
                operation,
                requested,
            } => {
                write!(
                    f,
                    "could not allocate {requested} bytes or records for {operation}"
                )
            }
            Self::UnclosedDevices {
                live_handles,
                teardown_uncertain,
            } => write!(
                f,
                "Finalize was skipped because {live_handles} device handle(s) remain live and teardown uncertainty is {teardown_uncertain}"
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::OpenFailedWithHandle { source, .. } => Some(source.as_ref()),
            _ => None,
        }
    }
}
