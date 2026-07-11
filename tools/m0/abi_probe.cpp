#include <cstddef>
#include <cstdio>
#include <string>
#include <type_traits>

#include "Mv3dLpApi.h"
#include "Mv3dLpDefine.h"
#include "Mv3dLpImgProc.h"

#ifndef WIN32
#error The ABI baseline must be compiled with WIN32, matching the vendor sample projects.
#endif

using MV3D_LP_PARAM_INFO = decltype(MV3D_LP_PARAM{}.ParamInfo);

#define ASSERT_FUNCTION_SIGNATURE(function_name, return_type, ...)                \
    static_assert(std::is_same_v<decltype(&function_name),                        \
                                 return_type(__cdecl*)(__VA_ARGS__)>,              \
                  "Unexpected signature for " #function_name)

#define ASSERT_CALLBACK_SIGNATURE(callback_name, ...)                             \
    static_assert(std::is_same_v<callback_name, void(__stdcall*)(__VA_ARGS__)>,   \
                  "Unexpected signature for " #callback_name)

ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetVersion, const char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_Initialize, MV3D_LP_STATUS);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_Finalize, MV3D_LP_STATUS);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetDeviceNumber, MV3D_LP_STATUS, uint32_t*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetDeviceList, MV3D_LP_STATUS,
                          MV3D_LP_DEVICE_INFO*, uint32_t, uint32_t*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_OpenDeviceByIP, MV3D_LP_STATUS, HANDLE*,
                          const char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_OpenDeviceBySN, MV3D_LP_STATUS, HANDLE*,
                          const char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_CloseDevice, MV3D_LP_STATUS, HANDLE*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_SetIpConfig, MV3D_LP_STATUS, const char*,
                          MV3D_LP_IP_CONFIG*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_RegisterExceptionCallBack, MV3D_LP_STATUS,
                          HANDLE, MV3D_LP_ExceptionCallBack, void*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_StartMeasure, MV3D_LP_STATUS, HANDLE);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_StopMeasure, MV3D_LP_STATUS, HANDLE);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_SoftTrigger, MV3D_LP_STATUS, HANDLE);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetImage, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_IMAGE_DATA*, uint32_t);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_RegisterImageDataCallBack, MV3D_LP_STATUS,
                          HANDLE, MV3D_LP_ImageDataCallBack, void*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_ClearDataBuffer, MV3D_LP_STATUS, HANDLE);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetParam, MV3D_LP_STATUS, HANDLE, const char*,
                          MV3D_LP_PARAM*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_SetParam, MV3D_LP_STATUS, HANDLE, const char*,
                          MV3D_LP_PARAM*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_Execute, MV3D_LP_STATUS, HANDLE, const char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_FileAccessRead, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_FILE_ACCESS*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_FileAccessWrite, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_FILE_ACCESS*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetFileAccessProgress, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_FILE_ACCESS_PROGRESS*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetDeviceIP, MV3D_LP_STATUS, uint32_t, char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetDeviceSN, MV3D_LP_STATUS, uint32_t, char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetProfile, MV3D_LP_STATUS, HANDLE, uint32_t,
                          MV3D_LP_PROFILE_DATA*, uint32_t);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetBatchProfile, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_DEPTH_DATA*, uint32_t);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_GetIntensityData, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_INTENSITY_DATA*, uint32_t);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_RegisterProfileCallBack, MV3D_LP_STATUS, HANDLE,
                          MV3D_LP_ProfileDataCallBack, uint32_t, void*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_RegisterBatchProfileCallBack, MV3D_LP_STATUS,
                          HANDLE, MV3D_LP_BatchProfileDataCallBack, void*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_MapDepthToPointCloud, MV3D_LP_STATUS,
                          MV3D_LP_IMAGE_DATA*, MV3D_LP_IMAGE_DATA*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_MapDepthToPointCloudRound, MV3D_LP_STATUS,
                          MV3D_LP_IMAGE_DATA*, uint32_t, MV3D_LP_IMAGE_DATA*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_ImageConvert, MV3D_LP_STATUS,
                          MV3D_LP_IMAGE_DATA*, MV3D_LP_IMAGE_DATA*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_DepthMosaic, MV3D_LP_STATUS,
                          MV3D_LP_IMAGE_DATA*, uint32_t, MV3D_LP_IMAGE_DATA*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_SaveImage, MV3D_LP_STATUS, MV3D_LP_IMAGE_DATA*,
                          Mv3dLpFileType, const char*);
ASSERT_FUNCTION_SIGNATURE(MV3D_LP_DisplayImage, MV3D_LP_STATUS,
                          MV3D_LP_IMAGE_DATA*, void*, Mv3dLpDisplayType, int32_t,
                          int32_t);

ASSERT_CALLBACK_SIGNATURE(MV3D_LP_ImageDataCallBack, MV3D_LP_IMAGE_DATA*, void*);
ASSERT_CALLBACK_SIGNATURE(MV3D_LP_ExceptionCallBack, MV3D_LP_EXCEPTION_INFO*,
                          void*);
ASSERT_CALLBACK_SIGNATURE(MV3D_LP_ProfileDataCallBack, MV3D_LP_PROFILE_DATA*,
                          MV3D_LP_INTENSITY_DATA*, void*);
ASSERT_CALLBACK_SIGNATURE(MV3D_LP_BatchProfileDataCallBack, MV3D_LP_DEPTH_DATA*,
                          MV3D_LP_INTENSITY_DATA*, void*);

#define TYPE_ENTRY(type_name)                                                     \
    std::printf("    \"" #type_name "\": {\"size\": %zu, \"align\": %zu}", \
                sizeof(type_name), alignof(type_name))

#define OFFSET_ENTRY(type_name, field_name)                                      \
    std::printf("    \"" #type_name "." #field_name "\": %zu",                 \
                offsetof(type_name, field_name))

#define HEX_VALUE_ENTRY(value_name)                                              \
    std::printf("    \"" #value_name "\": \"0x%08X\"",                         \
                static_cast<unsigned int>(value_name))

static void print_compiler()
{
    std::puts("  \"compiler\": {");
    std::printf("    \"msc_ver\": %d,\n", _MSC_VER);
    std::puts("    \"win32_macro\": true,");
    std::puts("    \"default_calling_convention\": \"cdecl (/Gd)\",");
    std::puts("    \"enum_underlying_type_signed\": {");
    std::printf("      \"Mv3dLpIpCfgMode\": %s,\n",
                std::is_signed_v<std::underlying_type_t<Mv3dLpIpCfgMode>>
                    ? "true"
                    : "false");
    std::printf("      \"Mv3dLpDevExceptionType\": %s,\n",
                std::is_signed_v<std::underlying_type_t<Mv3dLpDevExceptionType>>
                    ? "true"
                    : "false");
    std::printf("      \"Mv3dLpParamType\": %s,\n",
                std::is_signed_v<std::underlying_type_t<Mv3dLpParamType>>
                    ? "true"
                    : "false");
    std::printf("      \"Mv3dLpImageType\": %s,\n",
                std::is_signed_v<std::underlying_type_t<Mv3dLpImageType>>
                    ? "true"
                    : "false");
    std::printf("      \"Mv3dLpFileType\": %s,\n",
                std::is_signed_v<std::underlying_type_t<Mv3dLpFileType>>
                    ? "true"
                    : "false");
    std::printf("      \"Mv3dLpDisplayType\": %s\n",
                std::is_signed_v<std::underlying_type_t<Mv3dLpDisplayType>>
                    ? "true"
                    : "false");
    std::puts("    }");
    std::puts("  },");
}

static void print_values()
{
    std::puts("  \"values_u32_hex\": {");
    HEX_VALUE_ENTRY(MV3D_LP_UNDEFINED); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_OK); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_HANDLE); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_SUPPORT); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_BUFOVER); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_CALLORDER); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_PARAMETER); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_RESOURCE); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_NODATA); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_PRECONDITION); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_VERSION); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_NOENOUGH_BUF); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_ABNORMAL_IMAGE); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_LOAD_LIBRARY); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_ALGORITHM); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_DEVICE_OFFLINE); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_ACCESS_DENIED); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_OUTOFRANGE); std::puts(",");
    HEX_VALUE_ENTRY(MV3D_LP_E_UNKNOW); std::puts(",");
    HEX_VALUE_ENTRY(IpCfgMode_Static); std::puts(",");
    HEX_VALUE_ENTRY(IpCfgMode_DHCP); std::puts(",");
    HEX_VALUE_ENTRY(IpCfgMode_LLA); std::puts(",");
    HEX_VALUE_ENTRY(DevExceptionType_Undefined); std::puts(",");
    HEX_VALUE_ENTRY(DevExceptionType_Disconnect); std::puts(",");
    HEX_VALUE_ENTRY(ParamType_Undefined); std::puts(",");
    HEX_VALUE_ENTRY(ParamType_Bool); std::puts(",");
    HEX_VALUE_ENTRY(ParamType_Int); std::puts(",");
    HEX_VALUE_ENTRY(ParamType_Float); std::puts(",");
    HEX_VALUE_ENTRY(ParamType_Enum); std::puts(",");
    HEX_VALUE_ENTRY(ParamType_String); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_Undefined); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_Mono8); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_Depth); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_Profile); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_PointCloud); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_RGB24_Packed); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_Jpeg); std::puts(",");
    HEX_VALUE_ENTRY(ImageType_Profile_ABC32); std::puts(",");
    HEX_VALUE_ENTRY(FileType_Undefined); std::puts(",");
    HEX_VALUE_ENTRY(FileType_PLY); std::puts(",");
    HEX_VALUE_ENTRY(FileType_CSV); std::puts(",");
    HEX_VALUE_ENTRY(FileType_OBJ); std::puts(",");
    HEX_VALUE_ENTRY(FileType_BMP); std::puts(",");
    HEX_VALUE_ENTRY(FileType_JPG); std::puts(",");
    HEX_VALUE_ENTRY(FileType_TIFF); std::puts(",");
    HEX_VALUE_ENTRY(FileType_TIFF_U16); std::puts(",");
    HEX_VALUE_ENTRY(FileType_TIFF_F32); std::puts(",");
    HEX_VALUE_ENTRY(FileType_PLY_BINARY); std::puts(",");
    HEX_VALUE_ENTRY(FileType_PLY_TEXTURE); std::puts(",");
    HEX_VALUE_ENTRY(FileType_HIBAG); std::puts(",");
    HEX_VALUE_ENTRY(DisplayType_Undefined); std::puts(",");
    HEX_VALUE_ENTRY(DisplayType_Auto); std::puts(",");
    HEX_VALUE_ENTRY(DisplayType_Manual); std::putchar('\n');
    std::puts("  },");
}

static void print_type_layouts()
{
    std::puts("  \"types\": {");
    TYPE_ENTRY(MV3D_LP_STATUS); std::puts(",");
    TYPE_ENTRY(HANDLE); std::puts(",");
    TYPE_ENTRY(BOOL); std::puts(",");
    TYPE_ENTRY(Mv3dLpIpCfgMode); std::puts(",");
    TYPE_ENTRY(Mv3dLpDevExceptionType); std::puts(",");
    TYPE_ENTRY(Mv3dLpParamType); std::puts(",");
    TYPE_ENTRY(Mv3dLpImageType); std::puts(",");
    TYPE_ENTRY(Mv3dLpFileType); std::puts(",");
    TYPE_ENTRY(Mv3dLpDisplayType); std::puts(",");
    TYPE_ENTRY(MV3D_LP_DEVICE_INFO); std::puts(",");
    TYPE_ENTRY(MV3D_LP_IP_CONFIG); std::puts(",");
    TYPE_ENTRY(MV3D_LP_IMAGE_DATA); std::puts(",");
    TYPE_ENTRY(MV3D_LP_INTPARAM); std::puts(",");
    TYPE_ENTRY(MV3D_LP_ENUMPARAM); std::puts(",");
    TYPE_ENTRY(MV3D_LP_FLOATPARAM); std::puts(",");
    TYPE_ENTRY(MV3D_LP_STRINGPARAM); std::puts(",");
    TYPE_ENTRY(MV3D_LP_PARAM_INFO); std::puts(",");
    TYPE_ENTRY(MV3D_LP_PARAM); std::puts(",");
    TYPE_ENTRY(MV3D_LP_EXCEPTION_INFO); std::puts(",");
    TYPE_ENTRY(MV3D_LP_FILE_ACCESS); std::puts(",");
    TYPE_ENTRY(MV3D_LP_FILE_ACCESS_PROGRESS); std::puts(",");
    TYPE_ENTRY(MVB3D_LP_POINT_XYZ_S16); std::puts(",");
    TYPE_ENTRY(MVB3D_LP_POINT_XYZ_F32); std::puts(",");
    TYPE_ENTRY(MV3D_LP_PROFILE_DATA); std::puts(",");
    TYPE_ENTRY(MV3D_LP_DEPTH_DATA); std::puts(",");
    TYPE_ENTRY(MV3D_LP_INTENSITY_DATA); std::puts(",");
    TYPE_ENTRY(MV3D_LP_POINTCLOUD_DATA); std::puts(",");
    TYPE_ENTRY(MV3D_LP_ImageDataCallBack); std::puts(",");
    TYPE_ENTRY(MV3D_LP_ExceptionCallBack); std::puts(",");
    TYPE_ENTRY(MV3D_LP_ProfileDataCallBack); std::puts(",");
    TYPE_ENTRY(MV3D_LP_BatchProfileDataCallBack); std::putchar('\n');
    std::puts("  },");
}

static void print_offsets()
{
    std::puts("  \"offsets\": {");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chManufacturerName); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chModelName); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chDeviceVersion); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chManufacturerSpecificInfo); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chSerialNumber); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chUserDefinedName); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chMacAddress); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, enIPCfgMode); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chCurrentIp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chCurrentSubNetMask); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chDefultGateWay); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, chNetExport); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, nDevTypeInfo); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEVICE_INFO, nReserved); std::puts(",");

    OFFSET_ENTRY(MV3D_LP_IP_CONFIG, enIPCfgMode); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IP_CONFIG, chDestIp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IP_CONFIG, chDestNetMask); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IP_CONFIG, chDestGateWay); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IP_CONFIG, nReserved); std::puts(",");

    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, enImageType); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nWidth); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nHeight); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, pData); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nDataLen); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, pIntensityData); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nIntensityDataLen); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nFrameNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nTimeStamp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, bValid); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, fXScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, fYScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, fZScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nXOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nYOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nZOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, pExposureTimeStamp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_IMAGE_DATA, nReserved); std::puts(",");

    OFFSET_ENTRY(MV3D_LP_INTPARAM, nCurValue); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTPARAM, nMax); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTPARAM, nMin); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTPARAM, nInc); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_ENUMPARAM, nCurValue); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_ENUMPARAM, nSupportedNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_ENUMPARAM, nSupportValue); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FLOATPARAM, fCurValue); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FLOATPARAM, fMax); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FLOATPARAM, fMin); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_STRINGPARAM, chCurValue); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_STRINGPARAM, nMaxLength); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM_INFO, bBoolParam); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM_INFO, stIntParam); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM_INFO, stFloatParam); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM_INFO, stEnumParam); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM_INFO, stStringParam); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM, enParamType); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM, ParamInfo); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PARAM, nReserved); std::puts(",");

    OFFSET_ENTRY(MV3D_LP_EXCEPTION_INFO, enExceptionType); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_EXCEPTION_INFO, chExceptionDesc); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_EXCEPTION_INFO, nReserved); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FILE_ACCESS, pUserFileName); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FILE_ACCESS, pDevFileName); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FILE_ACCESS, nReserved); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FILE_ACCESS_PROGRESS, nCompleted); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FILE_ACCESS_PROGRESS, nTotal); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_FILE_ACCESS_PROGRESS, nReserved); std::puts(",");

    OFFSET_ENTRY(MVB3D_LP_POINT_XYZ_S16, nX); std::puts(",");
    OFFSET_ENTRY(MVB3D_LP_POINT_XYZ_S16, nY); std::puts(",");
    OFFSET_ENTRY(MVB3D_LP_POINT_XYZ_S16, nZ); std::puts(",");
    OFFSET_ENTRY(MVB3D_LP_POINT_XYZ_F32, fX); std::puts(",");
    OFFSET_ENTRY(MVB3D_LP_POINT_XYZ_F32, fY); std::puts(",");
    OFFSET_ENTRY(MVB3D_LP_POINT_XYZ_F32, fZ); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nLinePntNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nProfileCnt); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, pData); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nDataLen); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nFrameNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nTimeStamp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, bValid); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, fXScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, fYScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, fZScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nXOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nYOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nZOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_PROFILE_DATA, nReserved); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nWidth); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nHeight); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, pData); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nDataLen); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nFrameNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nTimeStamp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, bValid); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, fXScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, fYScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, fZScale); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nXOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nYOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nZOffset); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_DEPTH_DATA, nReserved); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, nWidth); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, nHeight); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, pData); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, nDataLen); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, nFrameNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, nTimeStamp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, bValid); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_INTENSITY_DATA, nReserved); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_POINTCLOUD_DATA, pData); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_POINTCLOUD_DATA, nDataLen); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_POINTCLOUD_DATA, nFrameNum); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_POINTCLOUD_DATA, nTimeStamp); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_POINTCLOUD_DATA, bValid); std::puts(",");
    OFFSET_ENTRY(MV3D_LP_POINTCLOUD_DATA, nReserved); std::putchar('\n');
    std::puts("  }");
}

int main()
{
#if defined(_WIN64)
    const char* architecture = "x64";
#else
    const char* architecture = "x86";
#endif

    const char* version_pointer = MV3D_LP_GetVersion();
    const std::string version =
        version_pointer == nullptr ? "<null>" : version_pointer;

    MV3D_LP_STATUS initialize_status = MV3D_LP_Initialize();
    MV3D_LP_STATUS get_device_number_status =
        static_cast<MV3D_LP_STATUS>(MV3D_LP_UNDEFINED);
    MV3D_LP_STATUS finalize_status =
        static_cast<MV3D_LP_STATUS>(MV3D_LP_UNDEFINED);
    if (initialize_status == MV3D_LP_OK) {
        uint32_t device_count = 0;
        get_device_number_status = MV3D_LP_GetDeviceNumber(&device_count);
        finalize_status = MV3D_LP_Finalize();
    }

    std::puts("{");
    std::printf("  \"architecture\": \"%s\",\n", architecture);
    std::printf("  \"pointer_size\": %zu,\n", sizeof(void*));
    std::printf("  \"sdk_version\": \"%s\",\n", version.c_str());
    std::printf(
        "  \"lifecycle\": {\"initialize_status\": \"0x%08X\", "
        "\"get_device_number_status\": \"0x%08X\", "
        "\"finalize_status\": \"0x%08X\"},\n",
        static_cast<unsigned int>(initialize_status),
        static_cast<unsigned int>(get_device_number_status),
        static_cast<unsigned int>(finalize_status));
    print_compiler();
    std::printf("  \"constants\": {\"max_string_length\": %u, \"max_enum_count\": %u},\n",
                static_cast<unsigned>(MV3D_LP_MAX_STRING_LENGTH),
                static_cast<unsigned>(MV3D_LP_MAX_ENUM_COUNT));
    print_values();
    print_type_layouts();
    print_offsets();
    std::puts("}");
    return 0;
}
