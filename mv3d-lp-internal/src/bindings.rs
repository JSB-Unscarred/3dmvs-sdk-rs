//! Audited raw bindings for the public 3DMVS LPSDK C headers.
//!
//! This module targets Windows x86_64 with the MSVC ABI only. The declarations
//! correspond to the three public headers shipped with LPSDK 1.3.3.3. Symbols
//! exported by the DLL but absent from those headers are intentionally omitted.

#![allow(
    dead_code,
    clippy::upper_case_acronyms,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]

use core::ffi::{c_char, c_void};

pub(crate) type MV3D_LP_STATUS = i32;
pub(crate) type HANDLE = *mut c_void;
pub(crate) type BOOL = i32;

pub(crate) const MV3D_LP_UNDEFINED: i32 = -1;

pub(crate) const MV3D_LP_OK: MV3D_LP_STATUS = 0;
pub(crate) const MV3D_LP_E_HANDLE: MV3D_LP_STATUS = 0x8006_0000_u32 as i32;
pub(crate) const MV3D_LP_E_SUPPORT: MV3D_LP_STATUS = 0x8006_0001_u32 as i32;
pub(crate) const MV3D_LP_E_BUFOVER: MV3D_LP_STATUS = 0x8006_0002_u32 as i32;
pub(crate) const MV3D_LP_E_CALLORDER: MV3D_LP_STATUS = 0x8006_0003_u32 as i32;
pub(crate) const MV3D_LP_E_PARAMETER: MV3D_LP_STATUS = 0x8006_0004_u32 as i32;
pub(crate) const MV3D_LP_E_RESOURCE: MV3D_LP_STATUS = 0x8006_0005_u32 as i32;
pub(crate) const MV3D_LP_E_NODATA: MV3D_LP_STATUS = 0x8006_0006_u32 as i32;
pub(crate) const MV3D_LP_E_PRECONDITION: MV3D_LP_STATUS = 0x8006_0007_u32 as i32;
pub(crate) const MV3D_LP_E_VERSION: MV3D_LP_STATUS = 0x8006_0008_u32 as i32;
pub(crate) const MV3D_LP_E_NOENOUGH_BUF: MV3D_LP_STATUS = 0x8006_0009_u32 as i32;
pub(crate) const MV3D_LP_E_ABNORMAL_IMAGE: MV3D_LP_STATUS = 0x8006_000A_u32 as i32;
pub(crate) const MV3D_LP_E_LOAD_LIBRARY: MV3D_LP_STATUS = 0x8006_000B_u32 as i32;
pub(crate) const MV3D_LP_E_ALGORITHM: MV3D_LP_STATUS = 0x8006_000C_u32 as i32;
pub(crate) const MV3D_LP_E_DEVICE_OFFLINE: MV3D_LP_STATUS = 0x8006_000D_u32 as i32;
pub(crate) const MV3D_LP_E_ACCESS_DENIED: MV3D_LP_STATUS = 0x8006_000E_u32 as i32;
pub(crate) const MV3D_LP_E_OUTOFRANGE: MV3D_LP_STATUS = 0x8006_000F_u32 as i32;
pub(crate) const MV3D_LP_E_UNKNOW: MV3D_LP_STATUS = 0x8006_00FF_u32 as i32;

pub(crate) const MV3D_LP_MAX_STRING_LENGTH: usize = 256;
pub(crate) const MV3D_LP_MAX_ENUM_COUNT: usize = 16;

pub(crate) const MV3D_LP_PIXEL_MONO: u32 = 0x0100_0000;
pub(crate) const MV3D_LP_PIXEL_COLOR: u32 = 0x0200_0000;
pub(crate) const MV3D_LP_PIXEL_CUSTOM: u32 = 0x8000_0000;

pub(crate) type Mv3dLpIpCfgMode = i32;
pub(crate) const IpCfgMode_Static: Mv3dLpIpCfgMode = 1;
pub(crate) const IpCfgMode_DHCP: Mv3dLpIpCfgMode = 2;
pub(crate) const IpCfgMode_LLA: Mv3dLpIpCfgMode = 4;

pub(crate) type Mv3dLpDevExceptionType = i32;
pub(crate) const DevExceptionType_Undefined: Mv3dLpDevExceptionType = -1;
pub(crate) const DevExceptionType_Disconnect: Mv3dLpDevExceptionType = 1;

pub(crate) type Mv3dLpParamType = i32;
pub(crate) const ParamType_Undefined: Mv3dLpParamType = -1;
pub(crate) const ParamType_Bool: Mv3dLpParamType = 1;
pub(crate) const ParamType_Int: Mv3dLpParamType = 2;
pub(crate) const ParamType_Float: Mv3dLpParamType = 3;
pub(crate) const ParamType_Enum: Mv3dLpParamType = 4;
pub(crate) const ParamType_String: Mv3dLpParamType = 5;

pub(crate) type Mv3dLpImageType = i32;
pub(crate) const ImageType_Undefined: Mv3dLpImageType = -1;
pub(crate) const ImageType_Mono8: Mv3dLpImageType = 0x0108_0001;
pub(crate) const ImageType_Depth: Mv3dLpImageType = 0x0110_00B8;
pub(crate) const ImageType_Profile: Mv3dLpImageType = 0x0230_00B9;
pub(crate) const ImageType_PointCloud: Mv3dLpImageType = 0x0260_00C0;
pub(crate) const ImageType_RGB24_Packed: Mv3dLpImageType = 0x0218_0014;
pub(crate) const ImageType_Jpeg: Mv3dLpImageType = 0x8018_0001_u32 as i32;
pub(crate) const ImageType_Profile_ABC32: Mv3dLpImageType = 0x8260_3001_u32 as i32;

pub(crate) type Mv3dLpFileType = i32;
pub(crate) const FileType_Undefined: Mv3dLpFileType = -1;
pub(crate) const FileType_PLY: Mv3dLpFileType = 1;
pub(crate) const FileType_CSV: Mv3dLpFileType = 2;
pub(crate) const FileType_OBJ: Mv3dLpFileType = 3;
pub(crate) const FileType_BMP: Mv3dLpFileType = 4;
pub(crate) const FileType_JPG: Mv3dLpFileType = 5;
pub(crate) const FileType_TIFF: Mv3dLpFileType = 6;
pub(crate) const FileType_TIFF_U16: Mv3dLpFileType = 7;
pub(crate) const FileType_TIFF_F32: Mv3dLpFileType = 8;
pub(crate) const FileType_PLY_BINARY: Mv3dLpFileType = 9;
pub(crate) const FileType_PLY_TEXTURE: Mv3dLpFileType = 10;
pub(crate) const FileType_HIBAG: Mv3dLpFileType = 11;

pub(crate) type Mv3dLpDisplayType = i32;
pub(crate) const DisplayType_Undefined: Mv3dLpDisplayType = -1;
pub(crate) const DisplayType_Auto: Mv3dLpDisplayType = 1;
pub(crate) const DisplayType_Manual: Mv3dLpDisplayType = 2;

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_DEVICE_INFO {
    pub(crate) chManufacturerName: [c_char; 32],
    pub(crate) chModelName: [c_char; 32],
    pub(crate) chDeviceVersion: [c_char; 32],
    pub(crate) chManufacturerSpecificInfo: [c_char; 48],
    pub(crate) chSerialNumber: [c_char; 16],
    pub(crate) chUserDefinedName: [c_char; 16],
    pub(crate) chMacAddress: [u8; 8],
    pub(crate) enIPCfgMode: Mv3dLpIpCfgMode,
    pub(crate) chCurrentIp: [c_char; 16],
    pub(crate) chCurrentSubNetMask: [c_char; 16],
    pub(crate) chDefultGateWay: [c_char; 16],
    pub(crate) chNetExport: [c_char; 16],
    pub(crate) nDevTypeInfo: u32,
    pub(crate) nReserved: [u8; 12],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_IP_CONFIG {
    pub(crate) enIPCfgMode: Mv3dLpIpCfgMode,
    pub(crate) chDestIp: [c_char; 16],
    pub(crate) chDestNetMask: [c_char; 16],
    pub(crate) chDestGateWay: [c_char; 16],
    pub(crate) nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_IMAGE_DATA {
    pub(crate) enImageType: Mv3dLpImageType,
    pub(crate) nWidth: u32,
    pub(crate) nHeight: u32,
    pub(crate) pData: *mut u8,
    pub(crate) nDataLen: u32,
    pub(crate) pIntensityData: *mut u8,
    pub(crate) nIntensityDataLen: u32,
    pub(crate) nFrameNum: u32,
    pub(crate) nTimeStamp: i64,
    pub(crate) bValid: BOOL,
    pub(crate) fXScale: f32,
    pub(crate) fYScale: f32,
    pub(crate) fZScale: f32,
    pub(crate) nXOffset: i32,
    pub(crate) nYOffset: i32,
    pub(crate) nZOffset: i32,
    pub(crate) pExposureTimeStamp: *mut i64,
    pub(crate) nReserved: [u8; 12],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_INTPARAM {
    pub(crate) nCurValue: i64,
    pub(crate) nMax: i64,
    pub(crate) nMin: i64,
    pub(crate) nInc: i64,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_ENUMPARAM {
    pub(crate) nCurValue: u32,
    pub(crate) nSupportedNum: u32,
    pub(crate) nSupportValue: [u32; MV3D_LP_MAX_ENUM_COUNT],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_FLOATPARAM {
    pub(crate) fCurValue: f32,
    pub(crate) fMax: f32,
    pub(crate) fMin: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_STRINGPARAM {
    pub(crate) chCurValue: [c_char; MV3D_LP_MAX_STRING_LENGTH],
    pub(crate) nMaxLength: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) union MV3D_LP_PARAM_INFO {
    pub(crate) bBoolParam: BOOL,
    pub(crate) stIntParam: MV3D_LP_INTPARAM,
    pub(crate) stFloatParam: MV3D_LP_FLOATPARAM,
    pub(crate) stEnumParam: MV3D_LP_ENUMPARAM,
    pub(crate) stStringParam: MV3D_LP_STRINGPARAM,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_PARAM {
    pub(crate) enParamType: Mv3dLpParamType,
    pub(crate) ParamInfo: MV3D_LP_PARAM_INFO,
    pub(crate) nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_EXCEPTION_INFO {
    pub(crate) enExceptionType: Mv3dLpDevExceptionType,
    pub(crate) chExceptionDesc: [c_char; MV3D_LP_MAX_STRING_LENGTH],
    pub(crate) nReserved: [u8; 4],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_FILE_ACCESS {
    pub(crate) pUserFileName: *const c_char,
    pub(crate) pDevFileName: *const c_char,
    pub(crate) nReserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_FILE_ACCESS_PROGRESS {
    pub(crate) nCompleted: i64,
    pub(crate) nTotal: i64,
    pub(crate) nReserved: [u8; 32],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MVB3D_LP_POINT_XYZ_S16 {
    pub(crate) nX: i16,
    pub(crate) nY: i16,
    pub(crate) nZ: i16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MVB3D_LP_POINT_XYZ_F32 {
    pub(crate) fX: f32,
    pub(crate) fY: f32,
    pub(crate) fZ: f32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_PROFILE_DATA {
    pub(crate) nLinePntNum: u32,
    pub(crate) nProfileCnt: u32,
    pub(crate) pData: *mut MVB3D_LP_POINT_XYZ_S16,
    pub(crate) nDataLen: u32,
    pub(crate) nFrameNum: u32,
    pub(crate) nTimeStamp: i64,
    pub(crate) bValid: BOOL,
    pub(crate) fXScale: f32,
    pub(crate) fYScale: f32,
    pub(crate) fZScale: f32,
    pub(crate) nXOffset: i32,
    pub(crate) nYOffset: i32,
    pub(crate) nZOffset: i32,
    pub(crate) nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_DEPTH_DATA {
    pub(crate) nWidth: u32,
    pub(crate) nHeight: u32,
    pub(crate) pData: *mut i16,
    pub(crate) nDataLen: u32,
    pub(crate) nFrameNum: u32,
    pub(crate) nTimeStamp: i64,
    pub(crate) bValid: BOOL,
    pub(crate) fXScale: f32,
    pub(crate) fYScale: f32,
    pub(crate) fZScale: f32,
    pub(crate) nXOffset: i32,
    pub(crate) nYOffset: i32,
    pub(crate) nZOffset: i32,
    pub(crate) nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_INTENSITY_DATA {
    pub(crate) nWidth: u32,
    pub(crate) nHeight: u32,
    pub(crate) pData: *mut u8,
    pub(crate) nDataLen: u32,
    pub(crate) nFrameNum: u32,
    pub(crate) nTimeStamp: i64,
    pub(crate) bValid: BOOL,
    pub(crate) nReserved: [u8; 16],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub(crate) struct MV3D_LP_POINTCLOUD_DATA {
    pub(crate) pData: *mut MVB3D_LP_POINT_XYZ_F32,
    pub(crate) nDataLen: u32,
    pub(crate) nFrameNum: u32,
    pub(crate) nTimeStamp: i64,
    pub(crate) bValid: BOOL,
    pub(crate) nReserved: [u8; 16],
}

pub(crate) type MV3D_LP_ImageDataCallBack =
    Option<unsafe extern "system" fn(*mut MV3D_LP_IMAGE_DATA, *mut c_void)>;
pub(crate) type MV3D_LP_ExceptionCallBack =
    Option<unsafe extern "system" fn(*mut MV3D_LP_EXCEPTION_INFO, *mut c_void)>;
pub(crate) type MV3D_LP_ProfileDataCallBack = Option<
    unsafe extern "system" fn(*mut MV3D_LP_PROFILE_DATA, *mut MV3D_LP_INTENSITY_DATA, *mut c_void),
>;
pub(crate) type MV3D_LP_BatchProfileDataCallBack = Option<
    unsafe extern "system" fn(*mut MV3D_LP_DEPTH_DATA, *mut MV3D_LP_INTENSITY_DATA, *mut c_void),
>;

unsafe extern "C" {
    pub(crate) fn MV3D_LP_GetVersion() -> *const c_char;
    pub(crate) fn MV3D_LP_Initialize() -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_Finalize() -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetDeviceNumber(pDeviceNumber: *mut u32) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetDeviceList(
        pstDeviceInfos: *mut MV3D_LP_DEVICE_INFO,
        nMaxDeviceCount: u32,
        pDeviceCount: *mut u32,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_OpenDeviceByIP(
        handle: *mut HANDLE,
        chIP: *const c_char,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_OpenDeviceBySN(
        handle: *mut HANDLE,
        chSN: *const c_char,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_CloseDevice(handle: *mut HANDLE) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_SetIpConfig(
        chSerialNumber: *const c_char,
        pstIPConfig: *mut MV3D_LP_IP_CONFIG,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_RegisterExceptionCallBack(
        handle: HANDLE,
        cbException: MV3D_LP_ExceptionCallBack,
        pUser: *mut c_void,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_StartMeasure(handle: HANDLE) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_StopMeasure(handle: HANDLE) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_SoftTrigger(handle: HANDLE) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetImage(
        handle: HANDLE,
        pstImageData: *mut MV3D_LP_IMAGE_DATA,
        nTimeout: u32,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_RegisterImageDataCallBack(
        handle: HANDLE,
        cbOutput: MV3D_LP_ImageDataCallBack,
        pUser: *mut c_void,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_ClearDataBuffer(handle: HANDLE) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetParam(
        handle: HANDLE,
        strKey: *const c_char,
        pstParam: *mut MV3D_LP_PARAM,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_SetParam(
        handle: HANDLE,
        strKey: *const c_char,
        pstParam: *mut MV3D_LP_PARAM,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_Execute(handle: HANDLE, strKey: *const c_char) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_FileAccessRead(
        handle: HANDLE,
        pstFileAccess: *mut MV3D_LP_FILE_ACCESS,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_FileAccessWrite(
        handle: HANDLE,
        pstFileAccess: *mut MV3D_LP_FILE_ACCESS,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetFileAccessProgress(
        handle: HANDLE,
        pstFileAccessProgress: *mut MV3D_LP_FILE_ACCESS_PROGRESS,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetDeviceIP(nDeviceIndex: u32, chIP: *mut c_char) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetDeviceSN(nDeviceIndex: u32, chSN: *mut c_char) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetProfile(
        handle: HANDLE,
        nProfileCount: u32,
        pstProfileData: *mut MV3D_LP_PROFILE_DATA,
        nTimeout: u32,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetBatchProfile(
        handle: HANDLE,
        pstDepthData: *mut MV3D_LP_DEPTH_DATA,
        nTimeout: u32,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_GetIntensityData(
        handle: HANDLE,
        pstIntensityData: *mut MV3D_LP_INTENSITY_DATA,
        nTimeout: u32,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_RegisterProfileCallBack(
        handle: HANDLE,
        cbOutput: MV3D_LP_ProfileDataCallBack,
        nProfileCount: u32,
        pUser: *mut c_void,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_RegisterBatchProfileCallBack(
        handle: HANDLE,
        cbOutput: MV3D_LP_BatchProfileDataCallBack,
        pUser: *mut c_void,
    ) -> MV3D_LP_STATUS;

    pub(crate) fn MV3D_LP_MapDepthToPointCloud(
        pstDepthImageData: *mut MV3D_LP_IMAGE_DATA,
        pstPointCloudData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_MapDepthToPointCloudRound(
        pstDepthDataList: *mut MV3D_LP_IMAGE_DATA,
        nImageCount: u32,
        pstPointCloudData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_ImageConvert(
        pstInImageData: *mut MV3D_LP_IMAGE_DATA,
        pstOutImageData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_DepthMosaic(
        pstDepthDataList: *mut MV3D_LP_IMAGE_DATA,
        nImageCount: u32,
        pstDepthData: *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_SaveImage(
        pstImage: *mut MV3D_LP_IMAGE_DATA,
        enFileType: Mv3dLpFileType,
        chFileName: *const c_char,
    ) -> MV3D_LP_STATUS;
    pub(crate) fn MV3D_LP_DisplayImage(
        pstImage: *mut MV3D_LP_IMAGE_DATA,
        hWnd: *mut c_void,
        enDisplayType: Mv3dLpDisplayType,
        nMin: i32,
        nMax: i32,
    ) -> MV3D_LP_STATUS;
}
