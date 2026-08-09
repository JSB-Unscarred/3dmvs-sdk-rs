use std::fmt;

/// Identifies the SDK operation associated with an error.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ContractViolation {
    NullVersionPointer,
    NullHandleOnSuccess,
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
    InvalidImageValue {
        field: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvalidInput {
    Empty,
    InteriorNul,
    TooLong {
        actual: usize,
        maximum: usize,
    },
    ImageCount {
        minimum: usize,
        maximum: usize,
        actual: usize,
    },
    InvalidImageLayout {
        field: &'static str,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    UnsupportedPlatform,
    RuntimeInactive,
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
    AllocationFailed {
        operation: Operation,
        requested: usize,
    },
    UnclosedDevices {
        live_handles: usize,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => f.write_str(
                "native 3DMVS support requires x86_64-pc-windows-msvc and the `native` feature",
            ),
            Self::RuntimeInactive => {
                f.write_str("this token no longer refers to the active 3DMVS runtime")
            }
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
            Self::AllocationFailed {
                operation,
                requested,
            } => {
                write!(
                    f,
                    "could not allocate {requested} bytes or records for {operation}"
                )
            }
            Self::UnclosedDevices { live_handles } => write!(
                f,
                "Finalize was skipped because {live_handles} device handle(s) remain live"
            ),
        }
    }
}

impl std::error::Error for Error {}
