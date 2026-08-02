use std::fmt;

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
    RuntimeDegraded,
    IncompatibleSdkVersion {
        minimum: &'static [u8],
        maximum_exclusive: Option<&'static [u8]>,
        actual: Vec<u8>,
    },
    Sdk {
        operation: Operation,
        status: i32,
    },
    InvalidState {
        operation: Operation,
        state: &'static str,
    },
    InvalidInput {
        operation: Operation,
        kind: InvalidInput,
    },
    ContractViolation {
        operation: Operation,
        kind: ContractViolation,
    },
    OpenFailedWithHandle {
        operation: Operation,
        source: Box<Error>,
    },
    DiscoveryChanged {
        attempts: usize,
    },
    AllocationFailed {
        operation: Operation,
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
            Self::RuntimeTerminal => f.write_str(
                "the 3DMVS runtime cannot accept this operation in its current lifecycle state",
            ),
            Self::RuntimeDegraded => f.write_str(
                "the process-wide 3DMVS SDK session is degraded and cannot open devices, finalize, or initialize another runtime",
            ),
            Self::IncompatibleSdkVersion {
                minimum,
                maximum_exclusive,
                actual,
            } => match maximum_exclusive {
                Some(maximum_exclusive) => write!(
                    f,
                    "incompatible 3DMVS runtime version: expected >= {} and < {}, got {}",
                    String::from_utf8_lossy(minimum),
                    String::from_utf8_lossy(maximum_exclusive),
                    String::from_utf8_lossy(actual)
                ),
                None => write!(
                    f,
                    "incompatible 3DMVS runtime version: expected exactly {}, got {}",
                    String::from_utf8_lossy(minimum),
                    String::from_utf8_lossy(actual)
                ),
            },
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
