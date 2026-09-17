use core::ffi::{c_char, c_void};

pub type MV3D_LP_STATUS = i32;
pub type HANDLE = *mut c_void;
pub type BOOL = i32;

pub const MV3D_LP_OK: MV3D_LP_STATUS = 0;
pub const MV3D_LP_E_HANDLE: MV3D_LP_STATUS = 0x8006_0000_u32.cast_signed();
pub const MV3D_LP_E_SUPPORT: MV3D_LP_STATUS = 0x8006_0001_u32.cast_signed();
pub const MV3D_LP_E_BUFOVER: MV3D_LP_STATUS = 0x8006_0002_u32.cast_signed();
pub const MV3D_LP_E_CALLORDER: MV3D_LP_STATUS = 0x8006_0003_u32.cast_signed();
pub const MV3D_LP_E_PARAMETER: MV3D_LP_STATUS = 0x8006_0004_u32.cast_signed();
pub const MV3D_LP_E_RESOURCE: MV3D_LP_STATUS = 0x8006_0005_u32.cast_signed();
pub const MV3D_LP_E_NODATA: MV3D_LP_STATUS = 0x8006_0006_u32.cast_signed();
pub const MV3D_LP_E_PRECONDITION: MV3D_LP_STATUS = 0x8006_0007_u32.cast_signed();
pub const MV3D_LP_E_VERSION: MV3D_LP_STATUS = 0x8006_0008_u32.cast_signed();
pub const MV3D_LP_E_NOENOUGH_BUF: MV3D_LP_STATUS = 0x8006_0009_u32.cast_signed();
pub const MV3D_LP_E_ABNORMAL_IMAGE: MV3D_LP_STATUS = 0x8006_000A_u32.cast_signed();
pub const MV3D_LP_E_LOAD_LIBRARY: MV3D_LP_STATUS = 0x8006_000B_u32.cast_signed();
pub const MV3D_LP_E_ALGORITHM: MV3D_LP_STATUS = 0x8006_000C_u32.cast_signed();
pub const MV3D_LP_E_DEVICE_OFFLINE: MV3D_LP_STATUS = 0x8006_000D_u32.cast_signed();
pub const MV3D_LP_E_ACCESS_DENIED: MV3D_LP_STATUS = 0x8006_000E_u32.cast_signed();
pub const MV3D_LP_E_OUTOFRANGE: MV3D_LP_STATUS = 0x8006_000F_u32.cast_signed();
pub const MV3D_LP_E_UNKNOW: MV3D_LP_STATUS = 0x8006_00FF_u32.cast_signed();

pub const MV3D_LP_MAX_STRING_LENGTH: usize = 256;
pub const MV3D_LP_MAX_ENUM_COUNT: usize = 16;

pub type Mv3dLpIpCfgMode = i32;
pub const IpCfgMode_Static: Mv3dLpIpCfgMode = 1;
pub const IpCfgMode_DHCP: Mv3dLpIpCfgMode = 2;
pub const IpCfgMode_LLA: Mv3dLpIpCfgMode = 4;

pub type Mv3dLpDevExceptionType = i32;
pub const DevExceptionType_Undefined: Mv3dLpDevExceptionType = -1;
pub const DevExceptionType_Disconnect: Mv3dLpDevExceptionType = 1;

pub type Mv3dLpParamType = i32;
pub const ParamType_Bool: Mv3dLpParamType = 1;
pub const ParamType_Int: Mv3dLpParamType = 2;
pub const ParamType_Float: Mv3dLpParamType = 3;
pub const ParamType_Enum: Mv3dLpParamType = 4;
pub const ParamType_String: Mv3dLpParamType = 5;

pub type Mv3dLpImageType = i32;
pub const ImageType_Undefined: Mv3dLpImageType = -1;
pub const ImageType_Mono8: Mv3dLpImageType = 0x0108_0001;
pub const ImageType_Depth: Mv3dLpImageType = 0x0110_00B8;
pub const ImageType_Profile: Mv3dLpImageType = 0x0230_00B9;
pub const ImageType_PointCloud: Mv3dLpImageType = 0x0260_00C0;
pub const ImageType_RGB24_Packed: Mv3dLpImageType = 0x0218_0014;
pub const ImageType_Jpeg: Mv3dLpImageType = 0x8018_0001_u32.cast_signed();
pub const ImageType_Profile_ABC32: Mv3dLpImageType = 0x8260_3001_u32.cast_signed();

pub type Mv3dLpFileType = i32;
pub const FileType_PLY: Mv3dLpFileType = 1;
pub const FileType_CSV: Mv3dLpFileType = 2;
pub const FileType_OBJ: Mv3dLpFileType = 3;
pub const FileType_BMP: Mv3dLpFileType = 4;
pub const FileType_JPG: Mv3dLpFileType = 5;
pub const FileType_TIFF: Mv3dLpFileType = 6;
pub const FileType_TIFF_U16: Mv3dLpFileType = 7;
pub const FileType_TIFF_F32: Mv3dLpFileType = 8;
pub const FileType_PLY_BINARY: Mv3dLpFileType = 9;
pub const FileType_PLY_TEXTURE: Mv3dLpFileType = 10;
pub const FileType_HIBAG: Mv3dLpFileType = 11;

pub type Mv3dLpDisplayType = i32;
pub const DisplayType_Auto: Mv3dLpDisplayType = 1;
pub const DisplayType_Manual: Mv3dLpDisplayType = 2;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_DEVICE_INFO {
    pub chManufacturerName: [c_char; 32],
    pub chModelName: [c_char; 32],
    pub chDeviceVersion: [c_char; 32],
    pub chManufacturerSpecificInfo: [c_char; 48],
    pub chSerialNumber: [c_char; 16],
    pub chUserDefinedName: [c_char; 16],
    pub chMacAddress: [u8; 8],
    pub enIPCfgMode: Mv3dLpIpCfgMode,
    pub chCurrentIp: [c_char; 16],
    pub chCurrentSubNetMask: [c_char; 16],
    pub chDefultGateWay: [c_char; 16],
    pub chNetExport: [c_char; 16],
    pub nDevTypeInfo: u32,
    pub nReserved: [u8; 12],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_IP_CONFIG {
    pub enIPCfgMode: Mv3dLpIpCfgMode,
    pub chDestIp: [c_char; 16],
    pub chDestNetMask: [c_char; 16],
    pub chDestGateWay: [c_char; 16],
    pub nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_IMAGE_DATA {
    pub enImageType: Mv3dLpImageType,
    pub nWidth: u32,
    pub nHeight: u32,
    pub pData: *mut u8,
    pub nDataLen: u32,
    pub pIntensityData: *mut u8,
    pub nIntensityDataLen: u32,
    pub nFrameNum: u32,
    pub nTimeStamp: i64,
    pub bValid: BOOL,
    pub fXScale: f32,
    pub fYScale: f32,
    pub fZScale: f32,
    pub nXOffset: i32,
    pub nYOffset: i32,
    pub nZOffset: i32,
    pub pExposureTimeStamp: *mut i64,
    pub nReserved: [u8; 12],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_INTPARAM {
    pub nCurValue: i64,
    pub nMax: i64,
    pub nMin: i64,
    pub nInc: i64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_ENUMPARAM {
    pub nCurValue: u32,
    pub nSupportedNum: u32,
    pub nSupportValue: [u32; MV3D_LP_MAX_ENUM_COUNT],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_FLOATPARAM {
    pub fCurValue: f32,
    pub fMax: f32,
    pub fMin: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_STRINGPARAM {
    pub chCurValue: [c_char; MV3D_LP_MAX_STRING_LENGTH],
    pub nMaxLength: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union MV3D_LP_PARAM_INFO {
    pub bBoolParam: BOOL,
    pub stIntParam: MV3D_LP_INTPARAM,
    pub stFloatParam: MV3D_LP_FLOATPARAM,
    pub stEnumParam: MV3D_LP_ENUMPARAM,
    pub stStringParam: MV3D_LP_STRINGPARAM,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_PARAM {
    pub enParamType: Mv3dLpParamType,
    pub ParamInfo: MV3D_LP_PARAM_INFO,
    pub nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_EXCEPTION_INFO {
    pub enExceptionType: Mv3dLpDevExceptionType,
    pub chExceptionDesc: [c_char; MV3D_LP_MAX_STRING_LENGTH],
    pub nReserved: [u8; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_FILE_ACCESS {
    pub pUserFileName: *const c_char,
    pub pDevFileName: *const c_char,
    pub nReserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct MV3D_LP_FILE_ACCESS_PROGRESS {
    pub nCompleted: i64,
    pub nTotal: i64,
    pub nReserved: [u8; 32],
}

pub type MV3D_LP_ImageDataCallBack =
    Option<unsafe extern "system" fn(*mut MV3D_LP_IMAGE_DATA, *mut c_void)>;
pub type MV3D_LP_ExceptionCallBack =
    Option<unsafe extern "system" fn(*mut MV3D_LP_EXCEPTION_INFO, *mut c_void)>;

unsafe extern "C" {
    pub fn MV3D_LP_GetVersion() -> *const c_char;
    pub fn MV3D_LP_Initialize() -> MV3D_LP_STATUS;
    pub fn MV3D_LP_Finalize() -> MV3D_LP_STATUS;
    pub fn MV3D_LP_GetDeviceNumber(pDeviceNumber: *mut u32) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_GetDeviceList(
        pstDeviceInfos: *mut MV3D_LP_DEVICE_INFO,
        nMaxDeviceCount: u32,
        pDeviceCount: *mut u32,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_OpenDeviceByIP(
        handle: *mut HANDLE,
        chIP: *const c_char,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_OpenDeviceBySN(
        handle: *mut HANDLE,
        chSN: *const c_char,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_CloseDevice(handle: *mut HANDLE) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_SetIpConfig(
        chSerialNumber: *const c_char,
        pstIPConfig: *mut MV3D_LP_IP_CONFIG,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_RegisterExceptionCallBack(
        handle: HANDLE,
        cbException: MV3D_LP_ExceptionCallBack,
        pUser: *mut c_void,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_StartMeasure(handle: HANDLE) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_StopMeasure(handle: HANDLE) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_SoftTrigger(handle: HANDLE) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_GetImage(
        handle: HANDLE,
        pstImageData: *mut MV3D_LP_IMAGE_DATA,
        nTimeout: u32,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_RegisterImageDataCallBack(
        handle: HANDLE,
        cbOutput: MV3D_LP_ImageDataCallBack,
        pUser: *mut c_void,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_ClearDataBuffer(handle: HANDLE) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_GetParam(
        handle: HANDLE,
        strKey: *const c_char,
        pstParam: *mut MV3D_LP_PARAM,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_SetParam(
        handle: HANDLE,
        strKey: *const c_char,
        pstParam: *mut MV3D_LP_PARAM,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_Execute(handle: HANDLE, strKey: *const c_char) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_FileAccessRead(
        handle: HANDLE,
        pstFileAccess: *mut MV3D_LP_FILE_ACCESS,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_FileAccessWrite(
        handle: HANDLE,
        pstFileAccess: *mut MV3D_LP_FILE_ACCESS,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_GetFileAccessProgress(
        handle: HANDLE,
        pstFileAccessProgress: *mut MV3D_LP_FILE_ACCESS_PROGRESS,
    ) -> MV3D_LP_STATUS;

    pub fn MV3D_LP_MapDepthToPointCloud(
        pstDepthImageData: *mut MV3D_LP_IMAGE_DATA,
        pstPointCloudData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_MapDepthToPointCloudRound(
        pstDepthDataList: *mut MV3D_LP_IMAGE_DATA,
        nImageCount: u32,
        pstPointCloudData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_ImageConvert(
        pstInImageData: *mut MV3D_LP_IMAGE_DATA,
        pstOutImageData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_DepthMosaic(
        pstDepthDataList: *mut MV3D_LP_IMAGE_DATA,
        nImageCount: u32,
        pstDepthData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_SaveImage(
        pstImage: *mut MV3D_LP_IMAGE_DATA,
        enFileType: Mv3dLpFileType,
        chFileName: *const c_char,
    ) -> MV3D_LP_STATUS;
    pub fn MV3D_LP_DisplayImage(
        pstImage: *mut MV3D_LP_IMAGE_DATA,
        hWnd: *mut c_void,
        enDisplayType: Mv3dLpDisplayType,
        nMin: i32,
        nMax: i32,
    ) -> MV3D_LP_STATUS;
}
