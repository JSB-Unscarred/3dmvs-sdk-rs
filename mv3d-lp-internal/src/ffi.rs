#![cfg_attr(not(feature = "native"), allow(dead_code, unused_imports))]

#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
use std::ffi::CStr;
use std::mem::{MaybeUninit, size_of};
#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
use std::ptr;

use crate::bindings;
#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
use crate::device::{DeviceInfoRaw, DeviceListAttempt, IpConfigRaw};
#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
use crate::driver::{Driver, Handle, status_result};
use crate::driver::{DriverError, DriverResult};
use crate::error::ContractViolation;
use crate::frame::{FrameRecord, ImageTypeRecord};
use crate::parameter::{ParameterRecord, ParameterValueRecord};

const VERSION_SCAN_LIMIT: usize = 64;
const DEFAULT_MAX_FRAME_BYTES: usize = 512 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrameLimits {
    max_frame_bytes: usize,
}

impl FrameLimits {
    #[cfg(test)]
    pub(crate) const fn with_max_frame_bytes(max_frame_bytes: usize) -> Self {
        Self { max_frame_bytes }
    }
}

impl Default for FrameLimits {
    fn default() -> Self {
        Self {
            max_frame_bytes: DEFAULT_MAX_FRAME_BYTES,
        }
    }
}

#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
pub(crate) struct NativeDriver;

#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
impl Driver for NativeDriver {
    fn version(&self) -> DriverResult<Vec<u8>> {
        // SAFETY: The linked LPSDK contract exposes this function without arguments.
        let pointer = unsafe { bindings::MV3D_LP_GetVersion() };
        if pointer.is_null() {
            return Err(DriverError::Contract(ContractViolation::NullVersionPointer));
        }

        let mut version = Vec::with_capacity(VERSION_SCAN_LIMIT);
        for index in 0..VERSION_SCAN_LIMIT {
            // SAFETY: LPSDK documents the returned pointer as a NUL-terminated version string.
            // We copy it immediately and cap the scan so it cannot become an unbounded Rust read.
            let byte = unsafe { pointer.add(index).read() } as u8;
            if byte == 0 {
                return Ok(version);
            }
            version.push(byte);
        }
        Err(DriverError::Contract(
            ContractViolation::UnterminatedVersion {
                limit: VERSION_SCAN_LIMIT,
            },
        ))
    }

    fn initialize(&self) -> DriverResult<()> {
        // SAFETY: Process lifecycle and serialization are enforced by Runtime's global gate.
        status_result(unsafe { bindings::MV3D_LP_Initialize() })
    }

    fn finalize(&self) -> DriverResult<()> {
        // SAFETY: Runtime calls Finalize once, after all borrowing Camera values are gone.
        status_result(unsafe { bindings::MV3D_LP_Finalize() })
    }

    fn device_number(&self) -> DriverResult<u32> {
        let mut count = 0;
        // SAFETY: count is a valid writable u32 for the duration of the call.
        status_result(unsafe { bindings::MV3D_LP_GetDeviceNumber(&mut count) })?;
        Ok(count)
    }

    fn device_list(&self, capacity: usize) -> DriverResult<DeviceListAttempt> {
        let native_capacity = u32::try_from(capacity).map_err(|_| {
            DriverError::Contract(ContractViolation::DeviceCountExceedsLimit {
                reported: u32::MAX,
                limit: capacity,
            })
        })?;
        let mut raw = Vec::new();
        raw.try_reserve_exact(capacity)
            .map_err(|_| DriverError::Allocation {
                requested: capacity,
            })?;
        raw.resize_with(capacity, zeroed_device_info);

        let mut reported = 0;
        // SAFETY: raw owns capacity initialized MV3D_LP_DEVICE_INFO values, and reported is a
        // valid writable scalar. Runtime holds the process-wide SDK call lock.
        let status = unsafe {
            bindings::MV3D_LP_GetDeviceList(raw.as_mut_ptr(), native_capacity, &mut reported)
        };
        status_result(status)?;

        let returned = usize::try_from(reported)
            .unwrap_or(usize::MAX)
            .min(raw.len());
        let records = raw
            .into_iter()
            .take(returned)
            .map(device_info_from_native)
            .collect();
        Ok(DeviceListAttempt { records, reported })
    }

    fn set_ip_config(&self, serial: &CStr, config: &IpConfigRaw) -> DriverResult<()> {
        let mut native = bindings::MV3D_LP_IP_CONFIG {
            enIPCfgMode: config.mode,
            chDestIp: as_c_char_array(&config.address),
            chDestNetMask: as_c_char_array(&config.subnet_mask),
            chDestGateWay: as_c_char_array(&config.gateway),
            nReserved: [0; 16],
        };
        // SAFETY: serial is NUL-terminated and borrowed for this call; native is fully
        // initialized, writable, and all reserved bytes are zero.
        status_result(unsafe { bindings::MV3D_LP_SetIpConfig(serial.as_ptr(), &mut native) })
    }

    fn open_by_ip(&self, ip: &CStr, handle: &mut Option<Handle>) -> DriverResult<()> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a valid writable handle slot and ip is NUL-terminated for the call.
        let status = unsafe { bindings::MV3D_LP_OpenDeviceByIP(&mut raw, ip.as_ptr()) };
        *handle = Handle::from_ptr(raw);
        status_result(status)
    }

    fn open_by_serial(&self, serial: &CStr, handle: &mut Option<Handle>) -> DriverResult<()> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a valid writable handle slot and serial is NUL-terminated for the call.
        let status = unsafe { bindings::MV3D_LP_OpenDeviceBySN(&mut raw, serial.as_ptr()) };
        *handle = Handle::from_ptr(raw);
        status_result(status)
    }

    fn close(&self, handle: Handle) -> DriverResult<()> {
        let mut raw = handle.as_ptr();
        // SAFETY: handle originated from a successful SDK open call. Camera removes it from its
        // state before entering this method, so Rust never calls CloseDevice twice for it.
        status_result(unsafe { bindings::MV3D_LP_CloseDevice(&mut raw) })
    }

    fn start(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Camera validates the state and owns this live SDK handle.
        status_result(unsafe { bindings::MV3D_LP_StartMeasure(handle.as_ptr()) })
    }

    fn stop(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Camera owns this live SDK handle; cleanup may conservatively call Stop after a
        // failed transition because the vendor does not define the partial state.
        status_result(unsafe { bindings::MV3D_LP_StopMeasure(handle.as_ptr()) })
    }

    fn soft_trigger(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Camera validates Measuring state and owns this live SDK handle.
        status_result(unsafe { bindings::MV3D_LP_SoftTrigger(handle.as_ptr()) })
    }

    fn clear_buffer(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Camera owns the handle and M2 exposes only copies, never borrowed SDK buffers.
        status_result(unsafe { bindings::MV3D_LP_ClearDataBuffer(handle.as_ptr()) })
    }

    fn get_image(&self, handle: Handle, timeout_ms: u32) -> DriverResult<FrameRecord> {
        let mut image = zeroed_image();
        // SAFETY: image is a fully zeroed writable SDK output, Camera owns the live handle, and
        // Runtime keeps the process-wide SDK gate locked until this method and its copies finish.
        let status = unsafe { bindings::MV3D_LP_GetImage(handle.as_ptr(), &mut image, timeout_ms) };
        // A failed SDK call does not initialize a trustworthy output descriptor. In particular,
        // never inspect or dereference pointer fields before checking the status.
        status_result(status)?;
        // SAFETY: On success the audited SDK contract guarantees that every non-null output
        // pointer remains readable for its reported extent until the immediate copy completes.
        // validate_image_layout checks lengths, null pairs, arithmetic, and the aggregate limit
        // before copy_image_from_native dereferences any pointer.
        unsafe { image_from_native(&image, FrameLimits::default()) }
    }

    fn get_parameter(&self, handle: Handle, key: &CStr) -> DriverResult<ParameterRecord> {
        let mut parameter = zeroed_parameter();
        // SAFETY: parameter is a fully zeroed writable output and key is NUL-terminated for the
        // call. The tagged union is read only after a successful status and discriminator check.
        status_result(unsafe {
            bindings::MV3D_LP_GetParam(handle.as_ptr(), key.as_ptr(), &mut parameter)
        })?;
        parameter_from_native(&parameter)
    }

    fn set_parameter(
        &self,
        handle: Handle,
        key: &CStr,
        value: &ParameterValueRecord,
    ) -> DriverResult<()> {
        let mut parameter = parameter_to_native(value)?;
        // SAFETY: key is NUL-terminated, parameter's active union member matches its
        // discriminator, and all inactive/reserved storage started zeroed.
        status_result(unsafe {
            bindings::MV3D_LP_SetParam(handle.as_ptr(), key.as_ptr(), &mut parameter)
        })
    }

    fn execute(&self, handle: Handle, key: &CStr) -> DriverResult<()> {
        // SAFETY: Camera owns this live handle and key is NUL-terminated for the call.
        status_result(unsafe { bindings::MV3D_LP_Execute(handle.as_ptr(), key.as_ptr()) })
    }
}

#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
fn zeroed_device_info() -> bindings::MV3D_LP_DEVICE_INFO {
    // SAFETY: The C structure consists only of integer scalars and byte arrays; all-zero is a
    // valid initialization pattern and is required by the SDK output contract.
    unsafe { MaybeUninit::zeroed().assume_init() }
}

pub(crate) fn zeroed_image() -> bindings::MV3D_LP_IMAGE_DATA {
    // SAFETY: The C structure consists of integer/float scalars, raw pointers, and a byte array;
    // all-zero is a valid initialization pattern and is required for this SDK output structure.
    unsafe { MaybeUninit::zeroed().assume_init() }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ValidatedImageLayout {
    data_len: usize,
    intensity_len: Option<usize>,
    exposure_count: Option<usize>,
    exposure_bytes: usize,
}

fn validate_image_layout(
    image: &bindings::MV3D_LP_IMAGE_DATA,
    limits: FrameLimits,
) -> DriverResult<ValidatedImageLayout> {
    if image.nWidth == 0 {
        return Err(invalid_image_value("width"));
    }
    if image.nHeight == 0 {
        return Err(invalid_image_value("height"));
    }

    let width = usize_from_u32(image.nWidth, "dimensions")?;
    let height = usize_from_u32(image.nHeight, "dimensions")?;
    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| length_overflow("dimensions"))?;
    let data_len = usize_from_u32(image.nDataLen, "data")?;

    // Pointer/length pairs are checked before format-specific arithmetic so a null pointer with
    // a claimed readable extent is always reported without trying to access it.
    if data_len != 0 && image.pData.is_null() {
        return Err(null_pointer_with_length("data", data_len));
    }
    if data_len == 0 {
        return Err(invalid_image_value("data length"));
    }

    if let Some(bytes_per_pixel) = known_bytes_per_pixel(image.enImageType) {
        let expected = pixels
            .checked_mul(bytes_per_pixel)
            .ok_or_else(|| length_overflow("data"))?;
        if data_len < expected {
            return Err(length_mismatch("data", expected, data_len));
        }
    }

    let intensity_len = usize_from_u32(image.nIntensityDataLen, "intensity data")?;
    let intensity_len = match intensity_len {
        0 => None,
        length if image.pIntensityData.is_null() => {
            return Err(null_pointer_with_length("intensity data", length));
        }
        length => {
            if length < pixels {
                return Err(length_mismatch("intensity data", pixels, length));
            }
            Some(length)
        }
    };

    let exposure_count = if image.pExposureTimeStamp.is_null() {
        None
    } else {
        Some(height)
    };
    let exposure_bytes = match exposure_count {
        Some(count) => count
            .checked_mul(size_of::<i64>())
            .ok_or_else(|| length_overflow("exposure timestamps"))?,
        None => 0,
    };

    let total = data_len
        .checked_add(intensity_len.unwrap_or(0))
        .and_then(|bytes| bytes.checked_add(exposure_bytes))
        .ok_or_else(|| length_overflow("frame payloads"))?;
    if total > limits.max_frame_bytes {
        return Err(DriverError::Contract(ContractViolation::OutputTooLarge {
            field: "frame payloads",
            limit: limits.max_frame_bytes,
            actual: total,
        }));
    }

    Ok(ValidatedImageLayout {
        data_len,
        intensity_len,
        exposure_count,
        exposure_bytes,
    })
}

unsafe fn image_from_native(
    image: &bindings::MV3D_LP_IMAGE_DATA,
    limits: FrameLimits,
) -> DriverResult<FrameRecord> {
    let layout = validate_image_layout(image, limits)?;

    // Reserve every destination before reading SDK memory. If any allocation fails, no vendor
    // pointer has been dereferenced and the call returns an ordinary allocation error.
    let mut data = allocated_bytes(layout.data_len)?;
    let mut intensity_data = match layout.intensity_len {
        Some(length) => Some(allocated_bytes(length)?),
        None => None,
    };
    let mut exposure_timestamps = match layout.exposure_count {
        Some(count) => Some(allocated_i64s(count, layout.exposure_bytes)?),
        None => None,
    };

    // SAFETY: validate_image_layout established a non-null data pointer and bounded length. The
    // caller of image_from_native guarantees that the SDK allocation is readable for that extent
    // until this immediate copy finishes.
    let source = unsafe { std::slice::from_raw_parts(image.pData.cast_const(), layout.data_len) };
    data.extend_from_slice(source);

    if let (Some(destination), Some(length)) = (&mut intensity_data, layout.intensity_len) {
        // SAFETY: The same conditions as for data apply to the optional intensity pointer, and
        // validate_image_layout additionally checked its exact pixel count.
        let source =
            unsafe { std::slice::from_raw_parts(image.pIntensityData.cast_const(), length) };
        destination.extend_from_slice(source);
    }

    if let (Some(destination), Some(count)) = (&mut exposure_timestamps, layout.exposure_count) {
        // Read exposure timestamps as bytes. This intentionally does not require the vendor's
        // pointer to satisfy Rust's i64 alignment even though its C type is int64_t*.
        // SAFETY: The caller guarantees `height * sizeof(i64)` readable bytes and validation
        // bounded that byte count before this slice is constructed.
        let bytes = unsafe {
            std::slice::from_raw_parts(
                image.pExposureTimeStamp.cast::<u8>().cast_const(),
                layout.exposure_bytes,
            )
        };
        for chunk in bytes.chunks_exact(size_of::<i64>()).take(count) {
            let encoded: [u8; size_of::<i64>()] = chunk
                .try_into()
                .expect("chunks_exact yields one native i64 at a time");
            destination.push(i64::from_ne_bytes(encoded));
        }
    }

    Ok(FrameRecord {
        image_type: ImageTypeRecord::from_raw(image.enImageType),
        width: image.nWidth,
        height: image.nHeight,
        data,
        intensity_data,
        exposure_timestamps,
        frame_number: image.nFrameNum,
        device_timestamp: image.nTimeStamp,
        valid: image.bValid != 0,
        x_scale: image.fXScale,
        y_scale: image.fYScale,
        z_scale: image.fZScale,
        x_offset: image.nXOffset,
        y_offset: image.nYOffset,
        z_offset: image.nZOffset,
    })
}

fn known_bytes_per_pixel(image_type: bindings::Mv3dLpImageType) -> Option<usize> {
    match image_type {
        bindings::ImageType_Mono8 => Some(1),
        bindings::ImageType_Depth => Some(2),
        bindings::ImageType_Profile => Some(6),
        bindings::ImageType_PointCloud | bindings::ImageType_Profile_ABC32 => Some(12),
        bindings::ImageType_RGB24_Packed => Some(3),
        bindings::ImageType_Jpeg | bindings::ImageType_Undefined => None,
        _ => None,
    }
}

fn allocated_bytes(length: usize) -> DriverResult<Vec<u8>> {
    let mut destination = Vec::new();
    destination
        .try_reserve_exact(length)
        .map_err(|_| DriverError::Allocation { requested: length })?;
    Ok(destination)
}

fn allocated_i64s(count: usize, requested_bytes: usize) -> DriverResult<Vec<i64>> {
    let mut destination = Vec::new();
    destination
        .try_reserve_exact(count)
        .map_err(|_| DriverError::Allocation {
            requested: requested_bytes,
        })?;
    Ok(destination)
}

fn usize_from_u32(value: u32, field: &'static str) -> DriverResult<usize> {
    usize::try_from(value).map_err(|_| length_overflow(field))
}

fn invalid_image_value(field: &'static str) -> DriverError {
    DriverError::Contract(ContractViolation::InvalidImageValue { field })
}

fn null_pointer_with_length(field: &'static str, length: usize) -> DriverError {
    DriverError::Contract(ContractViolation::NullPointerWithLength { field, length })
}

fn length_mismatch(field: &'static str, expected: usize, actual: usize) -> DriverError {
    DriverError::Contract(ContractViolation::LengthMismatch {
        field,
        expected,
        actual,
    })
}

fn length_overflow(field: &'static str) -> DriverError {
    DriverError::Contract(ContractViolation::LengthOverflow { field })
}

#[cfg(test)]
pub(crate) fn image_from_test_buffers(
    status: i32,
    mut image: bindings::MV3D_LP_IMAGE_DATA,
    data: Option<&[u8]>,
    intensity_data: Option<&[u8]>,
    exposure_timestamps: Option<&[i64]>,
    limits: FrameLimits,
) -> DriverResult<FrameRecord> {
    // Mirror NativeDriver: a failing status makes every output byte untrusted and must win over
    // malformed metadata or pointers.
    crate::driver::status_result(status)?;

    image.pData = data.map_or(std::ptr::null_mut(), |bytes| bytes.as_ptr().cast_mut());
    image.pIntensityData =
        intensity_data.map_or(std::ptr::null_mut(), |bytes| bytes.as_ptr().cast_mut());
    image.pExposureTimeStamp =
        exposure_timestamps.map_or(std::ptr::null_mut(), |values| values.as_ptr().cast_mut());

    let layout = validate_image_layout(&image, limits)?;
    if data.is_none_or(|bytes| bytes.len() < layout.data_len) {
        return Err(length_mismatch(
            "test data backing",
            layout.data_len,
            data.map_or(0, <[u8]>::len),
        ));
    }
    if let Some(expected) = layout.intensity_len
        && intensity_data.is_none_or(|bytes| bytes.len() < expected)
    {
        return Err(length_mismatch(
            "test intensity backing",
            expected,
            intensity_data.map_or(0, <[u8]>::len),
        ));
    }
    if let Some(expected) = layout.exposure_count
        && exposure_timestamps.is_none_or(|values| values.len() < expected)
    {
        return Err(length_mismatch(
            "test exposure backing",
            expected,
            exposure_timestamps.map_or(0, <[i64]>::len),
        ));
    }

    // SAFETY: Each pointer was derived from a live borrowed slice above, and the backing checks
    // guarantee the validated extents fit inside those slices for this synchronous call.
    unsafe { image_from_native(&image, limits) }
}

pub(crate) fn zeroed_parameter() -> bindings::MV3D_LP_PARAM {
    // SAFETY: The C tagged union and its containing integer/byte fields admit an all-zero bit
    // pattern. Zeroing the entire object also satisfies the SDK reserved-byte contract.
    unsafe { MaybeUninit::zeroed().assume_init() }
}

#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
fn device_info_from_native(native: bindings::MV3D_LP_DEVICE_INFO) -> DeviceInfoRaw {
    DeviceInfoRaw {
        manufacturer_name: as_u8_array(&native.chManufacturerName),
        model_name: as_u8_array(&native.chModelName),
        device_version: as_u8_array(&native.chDeviceVersion),
        manufacturer_specific_info: as_u8_array(&native.chManufacturerSpecificInfo),
        serial_number: as_u8_array(&native.chSerialNumber),
        user_defined_name: as_u8_array(&native.chUserDefinedName),
        mac_address: native.chMacAddress,
        ip_configuration_mode: native.enIPCfgMode,
        current_ip: as_u8_array(&native.chCurrentIp),
        current_subnet_mask: as_u8_array(&native.chCurrentSubNetMask),
        default_gateway: as_u8_array(&native.chDefultGateWay),
        interface_ip: as_u8_array(&native.chNetExport),
        device_type: native.nDevTypeInfo,
    }
}

pub(crate) fn parameter_from_native(
    parameter: &bindings::MV3D_LP_PARAM,
) -> DriverResult<ParameterRecord> {
    match parameter.enParamType {
        bindings::ParamType_Bool => {
            // SAFETY: enParamType identifies bBoolParam as the active union member.
            let value = unsafe { parameter.ParamInfo.bBoolParam };
            Ok(ParameterRecord::Bool(value != 0))
        }
        bindings::ParamType_Int => {
            // SAFETY: enParamType identifies stIntParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stIntParam };
            Ok(ParameterRecord::Integer {
                value: value.nCurValue,
                minimum: value.nMin,
                maximum: value.nMax,
                increment: value.nInc,
            })
        }
        bindings::ParamType_Float => {
            // SAFETY: enParamType identifies stFloatParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stFloatParam };
            Ok(ParameterRecord::Float {
                value: value.fCurValue,
                minimum: value.fMin,
                maximum: value.fMax,
            })
        }
        bindings::ParamType_Enum => {
            // SAFETY: enParamType identifies stEnumParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stEnumParam };
            let supported_count = usize::try_from(value.nSupportedNum).unwrap_or(usize::MAX);
            if supported_count > bindings::MV3D_LP_MAX_ENUM_COUNT {
                return Err(DriverError::Contract(
                    ContractViolation::EnumCountExceedsLimit {
                        reported: value.nSupportedNum,
                        limit: bindings::MV3D_LP_MAX_ENUM_COUNT,
                    },
                ));
            }
            Ok(ParameterRecord::Enumeration {
                value: value.nCurValue,
                supported: value.nSupportValue[..supported_count].to_vec(),
            })
        }
        bindings::ParamType_String => {
            // SAFETY: enParamType identifies stStringParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stStringParam };
            if usize::try_from(value.nMaxLength).unwrap_or(usize::MAX)
                > bindings::MV3D_LP_MAX_STRING_LENGTH
            {
                return Err(DriverError::Contract(
                    ContractViolation::StringMaxLengthExceedsLimit {
                        reported: value.nMaxLength,
                        limit: bindings::MV3D_LP_MAX_STRING_LENGTH,
                    },
                ));
            }
            let bytes = as_u8_array(&value.chCurValue);
            let length = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
            Ok(ParameterRecord::String {
                value: bytes[..length].to_vec(),
                maximum_length: value.nMaxLength,
            })
        }
        other => Err(DriverError::Contract(
            ContractViolation::UnknownParameterType(other),
        )),
    }
}

pub(crate) fn parameter_to_native(
    value: &ParameterValueRecord,
) -> DriverResult<bindings::MV3D_LP_PARAM> {
    let mut parameter = zeroed_parameter();
    match value {
        ParameterValueRecord::Bool(value) => {
            parameter.enParamType = bindings::ParamType_Bool;
            parameter.ParamInfo.bBoolParam = i32::from(*value);
        }
        ParameterValueRecord::Integer(value) => {
            parameter.enParamType = bindings::ParamType_Int;
            parameter.ParamInfo.stIntParam = bindings::MV3D_LP_INTPARAM {
                nCurValue: *value,
                nMax: 0,
                nMin: 0,
                nInc: 0,
            };
        }
        ParameterValueRecord::Float(value) => {
            parameter.enParamType = bindings::ParamType_Float;
            parameter.ParamInfo.stFloatParam = bindings::MV3D_LP_FLOATPARAM {
                fCurValue: *value,
                fMax: 0.0,
                fMin: 0.0,
            };
        }
        ParameterValueRecord::Enumeration(value) => {
            parameter.enParamType = bindings::ParamType_Enum;
            parameter.ParamInfo.stEnumParam = bindings::MV3D_LP_ENUMPARAM {
                nCurValue: *value,
                nSupportedNum: 0,
                nSupportValue: [0; bindings::MV3D_LP_MAX_ENUM_COUNT],
            };
        }
        ParameterValueRecord::String(value) => {
            if value.len() >= bindings::MV3D_LP_MAX_STRING_LENGTH {
                return Err(DriverError::Contract(
                    ContractViolation::StringMaxLengthExceedsLimit {
                        reported: u32::try_from(value.len()).unwrap_or(u32::MAX),
                        limit: bindings::MV3D_LP_MAX_STRING_LENGTH - 1,
                    },
                ));
            }
            if value.contains(&0) {
                return Err(DriverError::Status(bindings::MV3D_LP_E_PARAMETER));
            }
            let mut string = bindings::MV3D_LP_STRINGPARAM {
                chCurValue: [0; bindings::MV3D_LP_MAX_STRING_LENGTH],
                nMaxLength: 0,
            };
            for (destination, source) in string.chCurValue.iter_mut().zip(value) {
                *destination = *source as i8;
            }
            parameter.enParamType = bindings::ParamType_String;
            parameter.ParamInfo.stStringParam = string;
        }
    }
    if matches!(value, ParameterValueRecord::Bool(_)) {
        debug_assert!(bool_parameter_has_zeroed_inactive_storage(&parameter));
    }
    Ok(parameter)
}

fn as_u8_array<const N: usize>(source: &[i8; N]) -> [u8; N] {
    std::array::from_fn(|index| source[index] as u8)
}

#[cfg(all(
    feature = "native",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
fn as_c_char_array<const N: usize>(source: &[u8; N]) -> [i8; N] {
    std::array::from_fn(|index| source[index] as i8)
}

pub(crate) fn bool_parameter_has_zeroed_inactive_storage(
    parameter: &bindings::MV3D_LP_PARAM,
) -> bool {
    if parameter.enParamType != bindings::ParamType_Bool {
        return false;
    }
    // SAFETY: This audit is used only for a Bool-tagged parameter built from fully zeroed storage.
    // Every bit pattern of MV3D_LP_STRINGPARAM is valid, so viewing the initialized union storage
    // through its largest declared data member is valid here.
    let storage = unsafe { parameter.ParamInfo.stStringParam };
    let maximum_length = storage.nMaxLength;
    let storage = as_u8_array(&storage.chCurValue);
    let active = i32::from_ne_bytes([storage[0], storage[1], storage[2], storage[3]]);
    parameter.nReserved == [0; 16]
        && matches!(active, 0 | 1)
        && storage[4..].iter().all(|byte| *byte == 0)
        && maximum_length == 0
}
