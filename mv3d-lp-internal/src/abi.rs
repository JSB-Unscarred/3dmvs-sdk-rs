//! Compile-time layout checks for structures used by the safe wrapper.
//!
//! Sizes and offsets are copied from the audited Windows x86_64 MSVC headers. Constants and
//! function declarations are defined once in `bindings`; duplicating them here would not compare
//! against the vendor headers.

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
