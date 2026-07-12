//! Compile-time ABI lock for the audited Windows x86_64 MSVC baseline.

#![allow(dead_code)]

use core::ffi::{c_char, c_void};
use core::mem::{align_of, offset_of, size_of};

use crate::bindings::*;

macro_rules! assert_layout {
    ($ty:ty, $size:expr, $align:expr) => {
        const _: () = {
            assert!(size_of::<$ty>() == $size);
            assert!(align_of::<$ty>() == $align);
        };
    };
}

macro_rules! assert_offset {
    ($ty:ty, $field:ident, $offset:expr) => {
        const _: () = assert!(offset_of!($ty, $field) == $offset);
    };
}

assert_layout!(MV3D_LP_STATUS, 4, 4);
assert_layout!(HANDLE, 8, 8);
assert_layout!(BOOL, 4, 4);
assert_layout!(Mv3dLpIpCfgMode, 4, 4);
assert_layout!(Mv3dLpDevExceptionType, 4, 4);
assert_layout!(Mv3dLpParamType, 4, 4);
assert_layout!(Mv3dLpImageType, 4, 4);
assert_layout!(Mv3dLpFileType, 4, 4);
assert_layout!(Mv3dLpDisplayType, 4, 4);
assert_layout!(MV3D_LP_DEVICE_INFO, 268, 4);
assert_layout!(MV3D_LP_IP_CONFIG, 68, 4);
assert_layout!(MV3D_LP_IMAGE_DATA, 112, 8);
assert_layout!(MV3D_LP_INTPARAM, 32, 8);
assert_layout!(MV3D_LP_ENUMPARAM, 72, 4);
assert_layout!(MV3D_LP_FLOATPARAM, 12, 4);
assert_layout!(MV3D_LP_STRINGPARAM, 260, 4);
assert_layout!(MV3D_LP_PARAM_INFO, 264, 8);
assert_layout!(MV3D_LP_PARAM, 288, 8);
assert_layout!(MV3D_LP_EXCEPTION_INFO, 264, 4);
assert_layout!(MV3D_LP_FILE_ACCESS, 48, 8);
assert_layout!(MV3D_LP_FILE_ACCESS_PROGRESS, 48, 8);
assert_layout!(MVB3D_LP_POINT_XYZ_S16, 6, 2);
assert_layout!(MVB3D_LP_POINT_XYZ_F32, 12, 4);
assert_layout!(MV3D_LP_PROFILE_DATA, 80, 8);
assert_layout!(MV3D_LP_DEPTH_DATA, 80, 8);
assert_layout!(MV3D_LP_INTENSITY_DATA, 56, 8);
assert_layout!(MV3D_LP_POINTCLOUD_DATA, 48, 8);
assert_layout!(MV3D_LP_ImageDataCallBack, 8, 8);
assert_layout!(MV3D_LP_ExceptionCallBack, 8, 8);
assert_layout!(MV3D_LP_ProfileDataCallBack, 8, 8);
assert_layout!(MV3D_LP_BatchProfileDataCallBack, 8, 8);

assert_offset!(MV3D_LP_DEVICE_INFO, chManufacturerName, 0);
assert_offset!(MV3D_LP_DEVICE_INFO, chModelName, 32);
assert_offset!(MV3D_LP_DEVICE_INFO, chDeviceVersion, 64);
assert_offset!(MV3D_LP_DEVICE_INFO, chManufacturerSpecificInfo, 96);
assert_offset!(MV3D_LP_DEVICE_INFO, chSerialNumber, 144);
assert_offset!(MV3D_LP_DEVICE_INFO, chUserDefinedName, 160);
assert_offset!(MV3D_LP_DEVICE_INFO, chMacAddress, 176);
assert_offset!(MV3D_LP_DEVICE_INFO, enIPCfgMode, 184);
assert_offset!(MV3D_LP_DEVICE_INFO, chCurrentIp, 188);
assert_offset!(MV3D_LP_DEVICE_INFO, chCurrentSubNetMask, 204);
assert_offset!(MV3D_LP_DEVICE_INFO, chDefultGateWay, 220);
assert_offset!(MV3D_LP_DEVICE_INFO, chNetExport, 236);
assert_offset!(MV3D_LP_DEVICE_INFO, nDevTypeInfo, 252);
assert_offset!(MV3D_LP_DEVICE_INFO, nReserved, 256);

assert_offset!(MV3D_LP_IP_CONFIG, enIPCfgMode, 0);
assert_offset!(MV3D_LP_IP_CONFIG, chDestIp, 4);
assert_offset!(MV3D_LP_IP_CONFIG, chDestNetMask, 20);
assert_offset!(MV3D_LP_IP_CONFIG, chDestGateWay, 36);
assert_offset!(MV3D_LP_IP_CONFIG, nReserved, 52);

assert_offset!(MV3D_LP_IMAGE_DATA, enImageType, 0);
assert_offset!(MV3D_LP_IMAGE_DATA, nWidth, 4);
assert_offset!(MV3D_LP_IMAGE_DATA, nHeight, 8);
assert_offset!(MV3D_LP_IMAGE_DATA, pData, 16);
assert_offset!(MV3D_LP_IMAGE_DATA, nDataLen, 24);
assert_offset!(MV3D_LP_IMAGE_DATA, pIntensityData, 32);
assert_offset!(MV3D_LP_IMAGE_DATA, nIntensityDataLen, 40);
assert_offset!(MV3D_LP_IMAGE_DATA, nFrameNum, 44);
assert_offset!(MV3D_LP_IMAGE_DATA, nTimeStamp, 48);
assert_offset!(MV3D_LP_IMAGE_DATA, bValid, 56);
assert_offset!(MV3D_LP_IMAGE_DATA, fXScale, 60);
assert_offset!(MV3D_LP_IMAGE_DATA, fYScale, 64);
assert_offset!(MV3D_LP_IMAGE_DATA, fZScale, 68);
assert_offset!(MV3D_LP_IMAGE_DATA, nXOffset, 72);
assert_offset!(MV3D_LP_IMAGE_DATA, nYOffset, 76);
assert_offset!(MV3D_LP_IMAGE_DATA, nZOffset, 80);
assert_offset!(MV3D_LP_IMAGE_DATA, pExposureTimeStamp, 88);
assert_offset!(MV3D_LP_IMAGE_DATA, nReserved, 96);

assert_offset!(MV3D_LP_INTPARAM, nCurValue, 0);
assert_offset!(MV3D_LP_INTPARAM, nMax, 8);
assert_offset!(MV3D_LP_INTPARAM, nMin, 16);
assert_offset!(MV3D_LP_INTPARAM, nInc, 24);

assert_offset!(MV3D_LP_ENUMPARAM, nCurValue, 0);
assert_offset!(MV3D_LP_ENUMPARAM, nSupportedNum, 4);
assert_offset!(MV3D_LP_ENUMPARAM, nSupportValue, 8);

assert_offset!(MV3D_LP_FLOATPARAM, fCurValue, 0);
assert_offset!(MV3D_LP_FLOATPARAM, fMax, 4);
assert_offset!(MV3D_LP_FLOATPARAM, fMin, 8);

assert_offset!(MV3D_LP_STRINGPARAM, chCurValue, 0);
assert_offset!(MV3D_LP_STRINGPARAM, nMaxLength, 256);

assert_offset!(MV3D_LP_PARAM_INFO, bBoolParam, 0);
assert_offset!(MV3D_LP_PARAM_INFO, stIntParam, 0);
assert_offset!(MV3D_LP_PARAM_INFO, stFloatParam, 0);
assert_offset!(MV3D_LP_PARAM_INFO, stEnumParam, 0);
assert_offset!(MV3D_LP_PARAM_INFO, stStringParam, 0);

assert_offset!(MV3D_LP_PARAM, enParamType, 0);
assert_offset!(MV3D_LP_PARAM, ParamInfo, 8);
assert_offset!(MV3D_LP_PARAM, nReserved, 272);

assert_offset!(MV3D_LP_EXCEPTION_INFO, enExceptionType, 0);
assert_offset!(MV3D_LP_EXCEPTION_INFO, chExceptionDesc, 4);
assert_offset!(MV3D_LP_EXCEPTION_INFO, nReserved, 260);

assert_offset!(MV3D_LP_FILE_ACCESS, pUserFileName, 0);
assert_offset!(MV3D_LP_FILE_ACCESS, pDevFileName, 8);
assert_offset!(MV3D_LP_FILE_ACCESS, nReserved, 16);

assert_offset!(MV3D_LP_FILE_ACCESS_PROGRESS, nCompleted, 0);
assert_offset!(MV3D_LP_FILE_ACCESS_PROGRESS, nTotal, 8);
assert_offset!(MV3D_LP_FILE_ACCESS_PROGRESS, nReserved, 16);

assert_offset!(MVB3D_LP_POINT_XYZ_S16, nX, 0);
assert_offset!(MVB3D_LP_POINT_XYZ_S16, nY, 2);
assert_offset!(MVB3D_LP_POINT_XYZ_S16, nZ, 4);

assert_offset!(MVB3D_LP_POINT_XYZ_F32, fX, 0);
assert_offset!(MVB3D_LP_POINT_XYZ_F32, fY, 4);
assert_offset!(MVB3D_LP_POINT_XYZ_F32, fZ, 8);

assert_offset!(MV3D_LP_PROFILE_DATA, nLinePntNum, 0);
assert_offset!(MV3D_LP_PROFILE_DATA, nProfileCnt, 4);
assert_offset!(MV3D_LP_PROFILE_DATA, pData, 8);
assert_offset!(MV3D_LP_PROFILE_DATA, nDataLen, 16);
assert_offset!(MV3D_LP_PROFILE_DATA, nFrameNum, 20);
assert_offset!(MV3D_LP_PROFILE_DATA, nTimeStamp, 24);
assert_offset!(MV3D_LP_PROFILE_DATA, bValid, 32);
assert_offset!(MV3D_LP_PROFILE_DATA, fXScale, 36);
assert_offset!(MV3D_LP_PROFILE_DATA, fYScale, 40);
assert_offset!(MV3D_LP_PROFILE_DATA, fZScale, 44);
assert_offset!(MV3D_LP_PROFILE_DATA, nXOffset, 48);
assert_offset!(MV3D_LP_PROFILE_DATA, nYOffset, 52);
assert_offset!(MV3D_LP_PROFILE_DATA, nZOffset, 56);
assert_offset!(MV3D_LP_PROFILE_DATA, nReserved, 60);

assert_offset!(MV3D_LP_DEPTH_DATA, nWidth, 0);
assert_offset!(MV3D_LP_DEPTH_DATA, nHeight, 4);
assert_offset!(MV3D_LP_DEPTH_DATA, pData, 8);
assert_offset!(MV3D_LP_DEPTH_DATA, nDataLen, 16);
assert_offset!(MV3D_LP_DEPTH_DATA, nFrameNum, 20);
assert_offset!(MV3D_LP_DEPTH_DATA, nTimeStamp, 24);
assert_offset!(MV3D_LP_DEPTH_DATA, bValid, 32);
assert_offset!(MV3D_LP_DEPTH_DATA, fXScale, 36);
assert_offset!(MV3D_LP_DEPTH_DATA, fYScale, 40);
assert_offset!(MV3D_LP_DEPTH_DATA, fZScale, 44);
assert_offset!(MV3D_LP_DEPTH_DATA, nXOffset, 48);
assert_offset!(MV3D_LP_DEPTH_DATA, nYOffset, 52);
assert_offset!(MV3D_LP_DEPTH_DATA, nZOffset, 56);
assert_offset!(MV3D_LP_DEPTH_DATA, nReserved, 60);

assert_offset!(MV3D_LP_INTENSITY_DATA, nWidth, 0);
assert_offset!(MV3D_LP_INTENSITY_DATA, nHeight, 4);
assert_offset!(MV3D_LP_INTENSITY_DATA, pData, 8);
assert_offset!(MV3D_LP_INTENSITY_DATA, nDataLen, 16);
assert_offset!(MV3D_LP_INTENSITY_DATA, nFrameNum, 20);
assert_offset!(MV3D_LP_INTENSITY_DATA, nTimeStamp, 24);
assert_offset!(MV3D_LP_INTENSITY_DATA, bValid, 32);
assert_offset!(MV3D_LP_INTENSITY_DATA, nReserved, 36);

assert_offset!(MV3D_LP_POINTCLOUD_DATA, pData, 0);
assert_offset!(MV3D_LP_POINTCLOUD_DATA, nDataLen, 8);
assert_offset!(MV3D_LP_POINTCLOUD_DATA, nFrameNum, 12);
assert_offset!(MV3D_LP_POINTCLOUD_DATA, nTimeStamp, 16);
assert_offset!(MV3D_LP_POINTCLOUD_DATA, bValid, 24);
assert_offset!(MV3D_LP_POINTCLOUD_DATA, nReserved, 28);

const _: () = {
    assert!(MV3D_LP_MAX_STRING_LENGTH == 256);
    assert!(MV3D_LP_MAX_ENUM_COUNT == 16);
    assert!(MV3D_LP_UNDEFINED as u32 == 0xFFFF_FFFF);
    assert!(MV3D_LP_OK as u32 == 0x0000_0000);
    assert!(MV3D_LP_E_HANDLE as u32 == 0x8006_0000);
    assert!(MV3D_LP_E_SUPPORT as u32 == 0x8006_0001);
    assert!(MV3D_LP_E_BUFOVER as u32 == 0x8006_0002);
    assert!(MV3D_LP_E_CALLORDER as u32 == 0x8006_0003);
    assert!(MV3D_LP_E_PARAMETER as u32 == 0x8006_0004);
    assert!(MV3D_LP_E_RESOURCE as u32 == 0x8006_0005);
    assert!(MV3D_LP_E_NODATA as u32 == 0x8006_0006);
    assert!(MV3D_LP_E_PRECONDITION as u32 == 0x8006_0007);
    assert!(MV3D_LP_E_VERSION as u32 == 0x8006_0008);
    assert!(MV3D_LP_E_NOENOUGH_BUF as u32 == 0x8006_0009);
    assert!(MV3D_LP_E_ABNORMAL_IMAGE as u32 == 0x8006_000A);
    assert!(MV3D_LP_E_LOAD_LIBRARY as u32 == 0x8006_000B);
    assert!(MV3D_LP_E_ALGORITHM as u32 == 0x8006_000C);
    assert!(MV3D_LP_E_DEVICE_OFFLINE as u32 == 0x8006_000D);
    assert!(MV3D_LP_E_ACCESS_DENIED as u32 == 0x8006_000E);
    assert!(MV3D_LP_E_OUTOFRANGE as u32 == 0x8006_000F);
    assert!(MV3D_LP_E_UNKNOW as u32 == 0x8006_00FF);
    assert!(IpCfgMode_Static as u32 == 0x0000_0001);
    assert!(IpCfgMode_DHCP as u32 == 0x0000_0002);
    assert!(IpCfgMode_LLA as u32 == 0x0000_0004);
    assert!(DevExceptionType_Undefined as u32 == 0xFFFF_FFFF);
    assert!(DevExceptionType_Disconnect as u32 == 0x0000_0001);
    assert!(ParamType_Undefined as u32 == 0xFFFF_FFFF);
    assert!(ParamType_Bool as u32 == 0x0000_0001);
    assert!(ParamType_Int as u32 == 0x0000_0002);
    assert!(ParamType_Float as u32 == 0x0000_0003);
    assert!(ParamType_Enum as u32 == 0x0000_0004);
    assert!(ParamType_String as u32 == 0x0000_0005);
    assert!(ImageType_Undefined as u32 == 0xFFFF_FFFF);
    assert!(ImageType_Mono8 as u32 == 0x0108_0001);
    assert!(ImageType_Depth as u32 == 0x0110_00B8);
    assert!(ImageType_Profile as u32 == 0x0230_00B9);
    assert!(ImageType_PointCloud as u32 == 0x0260_00C0);
    assert!(ImageType_RGB24_Packed as u32 == 0x0218_0014);
    assert!(ImageType_Jpeg as u32 == 0x8018_0001);
    assert!(ImageType_Profile_ABC32 as u32 == 0x8260_3001);
    assert!(FileType_Undefined as u32 == 0xFFFF_FFFF);
    assert!(FileType_PLY as u32 == 0x0000_0001);
    assert!(FileType_CSV as u32 == 0x0000_0002);
    assert!(FileType_OBJ as u32 == 0x0000_0003);
    assert!(FileType_BMP as u32 == 0x0000_0004);
    assert!(FileType_JPG as u32 == 0x0000_0005);
    assert!(FileType_TIFF as u32 == 0x0000_0006);
    assert!(FileType_TIFF_U16 as u32 == 0x0000_0007);
    assert!(FileType_TIFF_F32 as u32 == 0x0000_0008);
    assert!(FileType_PLY_BINARY as u32 == 0x0000_0009);
    assert!(FileType_PLY_TEXTURE as u32 == 0x0000_000A);
    assert!(FileType_HIBAG as u32 == 0x0000_000B);
    assert!(DisplayType_Undefined as u32 == 0xFFFF_FFFF);
    assert!(DisplayType_Auto as u32 == 0x0000_0001);
    assert!(DisplayType_Manual as u32 == 0x0000_0002);
    assert!(MV3D_LP_PIXEL_MONO == 0x0100_0000);
    assert!(MV3D_LP_PIXEL_COLOR == 0x0200_0000);
    assert!(MV3D_LP_PIXEL_CUSTOM == 0x8000_0000);
};

fn assert_callback_signatures() {
    let _: MV3D_LP_ImageDataCallBack =
        None::<unsafe extern "system" fn(*mut MV3D_LP_IMAGE_DATA, *mut c_void)>;
    let _: MV3D_LP_ExceptionCallBack =
        None::<unsafe extern "system" fn(*mut MV3D_LP_EXCEPTION_INFO, *mut c_void)>;
    let _: MV3D_LP_ProfileDataCallBack = None::<
        unsafe extern "system" fn(
            *mut MV3D_LP_PROFILE_DATA,
            *mut MV3D_LP_INTENSITY_DATA,
            *mut c_void,
        ),
    >;
    let _: MV3D_LP_BatchProfileDataCallBack = None::<
        unsafe extern "system" fn(
            *mut MV3D_LP_DEPTH_DATA,
            *mut MV3D_LP_INTENSITY_DATA,
            *mut c_void,
        ),
    >;
}

fn assert_function_signatures() {
    let _: unsafe extern "C" fn() -> *const c_char = MV3D_LP_GetVersion;
    let _: unsafe extern "C" fn() -> MV3D_LP_STATUS = MV3D_LP_Initialize;
    let _: unsafe extern "C" fn() -> MV3D_LP_STATUS = MV3D_LP_Finalize;
    let _: unsafe extern "C" fn(*mut u32) -> MV3D_LP_STATUS = MV3D_LP_GetDeviceNumber;
    let _: unsafe extern "C" fn(*mut MV3D_LP_DEVICE_INFO, u32, *mut u32) -> MV3D_LP_STATUS =
        MV3D_LP_GetDeviceList;
    let _: unsafe extern "C" fn(*mut HANDLE, *const c_char) -> MV3D_LP_STATUS =
        MV3D_LP_OpenDeviceByIP;
    let _: unsafe extern "C" fn(*mut HANDLE, *const c_char) -> MV3D_LP_STATUS =
        MV3D_LP_OpenDeviceBySN;
    let _: unsafe extern "C" fn(*mut HANDLE) -> MV3D_LP_STATUS = MV3D_LP_CloseDevice;
    let _: unsafe extern "C" fn(*const c_char, *mut MV3D_LP_IP_CONFIG) -> MV3D_LP_STATUS =
        MV3D_LP_SetIpConfig;
    let _: unsafe extern "C" fn(HANDLE, MV3D_LP_ExceptionCallBack, *mut c_void) -> MV3D_LP_STATUS =
        MV3D_LP_RegisterExceptionCallBack;
    let _: unsafe extern "C" fn(HANDLE) -> MV3D_LP_STATUS = MV3D_LP_StartMeasure;
    let _: unsafe extern "C" fn(HANDLE) -> MV3D_LP_STATUS = MV3D_LP_StopMeasure;
    let _: unsafe extern "C" fn(HANDLE) -> MV3D_LP_STATUS = MV3D_LP_SoftTrigger;
    let _: unsafe extern "C" fn(HANDLE, *mut MV3D_LP_IMAGE_DATA, u32) -> MV3D_LP_STATUS =
        MV3D_LP_GetImage;
    let _: unsafe extern "C" fn(HANDLE, MV3D_LP_ImageDataCallBack, *mut c_void) -> MV3D_LP_STATUS =
        MV3D_LP_RegisterImageDataCallBack;
    let _: unsafe extern "C" fn(HANDLE) -> MV3D_LP_STATUS = MV3D_LP_ClearDataBuffer;
    let _: unsafe extern "C" fn(HANDLE, *const c_char, *mut MV3D_LP_PARAM) -> MV3D_LP_STATUS =
        MV3D_LP_GetParam;
    let _: unsafe extern "C" fn(HANDLE, *const c_char, *mut MV3D_LP_PARAM) -> MV3D_LP_STATUS =
        MV3D_LP_SetParam;
    let _: unsafe extern "C" fn(HANDLE, *const c_char) -> MV3D_LP_STATUS = MV3D_LP_Execute;
    let _: unsafe extern "C" fn(HANDLE, *mut MV3D_LP_FILE_ACCESS) -> MV3D_LP_STATUS =
        MV3D_LP_FileAccessRead;
    let _: unsafe extern "C" fn(HANDLE, *mut MV3D_LP_FILE_ACCESS) -> MV3D_LP_STATUS =
        MV3D_LP_FileAccessWrite;
    let _: unsafe extern "C" fn(HANDLE, *mut MV3D_LP_FILE_ACCESS_PROGRESS) -> MV3D_LP_STATUS =
        MV3D_LP_GetFileAccessProgress;
    let _: unsafe extern "C" fn(u32, *mut c_char) -> MV3D_LP_STATUS = MV3D_LP_GetDeviceIP;
    let _: unsafe extern "C" fn(u32, *mut c_char) -> MV3D_LP_STATUS = MV3D_LP_GetDeviceSN;
    let _: unsafe extern "C" fn(HANDLE, u32, *mut MV3D_LP_PROFILE_DATA, u32) -> MV3D_LP_STATUS =
        MV3D_LP_GetProfile;
    let _: unsafe extern "C" fn(HANDLE, *mut MV3D_LP_DEPTH_DATA, u32) -> MV3D_LP_STATUS =
        MV3D_LP_GetBatchProfile;
    let _: unsafe extern "C" fn(HANDLE, *mut MV3D_LP_INTENSITY_DATA, u32) -> MV3D_LP_STATUS =
        MV3D_LP_GetIntensityData;
    let _: unsafe extern "C" fn(
        HANDLE,
        MV3D_LP_ProfileDataCallBack,
        u32,
        *mut c_void,
    ) -> MV3D_LP_STATUS = MV3D_LP_RegisterProfileCallBack;
    let _: unsafe extern "C" fn(
        HANDLE,
        MV3D_LP_BatchProfileDataCallBack,
        *mut c_void,
    ) -> MV3D_LP_STATUS = MV3D_LP_RegisterBatchProfileCallBack;
    let _: unsafe extern "C" fn(
        *mut MV3D_LP_IMAGE_DATA,
        *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS = MV3D_LP_MapDepthToPointCloud;
    let _: unsafe extern "C" fn(
        *mut MV3D_LP_IMAGE_DATA,
        u32,
        *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS = MV3D_LP_MapDepthToPointCloudRound;
    let _: unsafe extern "C" fn(
        *mut MV3D_LP_IMAGE_DATA,
        *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS = MV3D_LP_ImageConvert;
    let _: unsafe extern "C" fn(
        *mut MV3D_LP_IMAGE_DATA,
        u32,
        *mut MV3D_LP_IMAGE_DATA,
    ) -> MV3D_LP_STATUS = MV3D_LP_DepthMosaic;
    let _: unsafe extern "C" fn(
        *mut MV3D_LP_IMAGE_DATA,
        Mv3dLpFileType,
        *const c_char,
    ) -> MV3D_LP_STATUS = MV3D_LP_SaveImage;
    let _: unsafe extern "C" fn(
        *mut MV3D_LP_IMAGE_DATA,
        *mut c_void,
        Mv3dLpDisplayType,
        i32,
        i32,
    ) -> MV3D_LP_STATUS = MV3D_LP_DisplayImage;
}
