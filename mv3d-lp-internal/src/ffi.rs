#![cfg_attr(not(feature = "native"), allow(dead_code, unused_imports))]

use std::ffi::CStr;
use std::mem::{MaybeUninit, size_of};
#[cfg(all(native_sdk, feature = "display-windows"))]
use std::num::NonZeroIsize;
use std::ptr;

use crate::bindings;
use crate::callback::{CallbackCookie, exception_trampoline, image_trampoline};
use crate::device::{DeviceInfo, IpConfigRaw, parse_optional_ipv4};
#[cfg(all(native_sdk, feature = "display-windows"))]
use crate::display::DisplayRange;
use crate::driver::{DriverError, DriverResult, Handle, status_result};
use crate::error::{ContractViolation, InputViolation};
use crate::file_transfer::FileProgress;
use crate::frame::{Image, ImageCalibration, ImageFileFormat, ImageRef, ImageType};
use crate::parameter::{Parameter, ParameterValue};
use crate::text::{SdkText, SerialNumber};

const MAX_MULTI_IMAGE_COUNT: usize = 8;

/// Concrete native call boundary; lifecycle and ownership stay in the safe wrapper.
pub(crate) struct NativeDriver;

#[cfg(native_sdk)]
impl NativeDriver {
    pub(crate) fn version(&self) -> DriverResult<Vec<u8>> {
        // SAFETY: The linked LPSDK contract exposes this function without arguments.
        let pointer = unsafe { bindings::MV3D_LP_GetVersion() };
        if pointer.is_null() {
            return Err(DriverError::Contract(ContractViolation::NullPointer {
                field: "SDK version",
            }));
        }

        // SAFETY: LPSDK documents the returned pointer as a NUL-terminated version string.
        Ok(unsafe { CStr::from_ptr(pointer) }.to_bytes().to_vec())
    }

    pub(crate) fn initialize(&self) -> DriverResult<()> {
        // SAFETY: Runtime admits Initialize only once during the process lifetime.
        status_result(unsafe { bindings::MV3D_LP_Initialize() })
    }

    pub(crate) fn finalize(&self) -> DriverResult<()> {
        // SAFETY: Runtime consumes the sole session owner before calling Finalize once.
        status_result(unsafe { bindings::MV3D_LP_Finalize() })
    }

    pub(crate) fn device_number(&self) -> DriverResult<u32> {
        let mut count = 0;
        // SAFETY: count is a valid writable u32 for the duration of the call.
        status_result(unsafe { bindings::MV3D_LP_GetDeviceNumber(&mut count) })?;
        Ok(count)
    }

    pub(crate) fn device_list(&self, capacity: usize) -> DriverResult<Vec<DeviceInfo>> {
        let native_capacity = u32::try_from(capacity).map_err(|_| {
            DriverError::Contract(ContractViolation::LengthOverflow {
                field: "device list capacity",
            })
        })?;
        let mut raw = Vec::with_capacity(capacity);
        raw.resize_with(capacity, zeroed_device_info);

        let mut reported = 0;
        // SAFETY: raw owns capacity initialized MV3D_LP_DEVICE_INFO values, and reported is a
        // valid writable scalar. Both remain exclusively borrowed for this synchronous call.
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
        Ok(records)
    }

    pub(crate) fn set_ip_config(&self, serial: &CStr, config: &IpConfigRaw) -> DriverResult<()> {
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

    pub(crate) fn open_by_ip(&self, ip: &CStr) -> DriverResult<Handle> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a valid writable handle slot and ip is NUL-terminated for the call.
        let status = unsafe { bindings::MV3D_LP_OpenDeviceByIP(&mut raw, ip.as_ptr()) };
        status_result(status)?;
        Handle::from_ptr(raw).ok_or(DriverError::Contract(ContractViolation::NullPointer {
            field: "device handle",
        }))
    }

    pub(crate) fn open_by_serial(&self, serial: &CStr) -> DriverResult<Handle> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a valid writable handle slot and serial is NUL-terminated for the call.
        let status = unsafe { bindings::MV3D_LP_OpenDeviceBySN(&mut raw, serial.as_ptr()) };
        status_result(status)?;
        Handle::from_ptr(raw).ok_or(DriverError::Contract(ContractViolation::NullPointer {
            field: "device handle",
        }))
    }

    pub(crate) fn close(&self, handle: Handle) -> DriverResult<()> {
        let mut raw = handle.as_ptr();
        // SAFETY: handle originated from a successful SDK open call and its Device owner calls
        // CloseDevice at most once. Returning consumes the handle even when status reports an
        // error; the SDK has also quiesced callbacks and released asynchronous input borrows.
        status_result(unsafe { bindings::MV3D_LP_CloseDevice(&mut raw) })
    }

    pub(crate) fn start(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device validates the state and owns this live SDK handle.
        status_result(unsafe { bindings::MV3D_LP_StartMeasure(handle.as_ptr()) })
    }

    pub(crate) fn stop(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device owns this live SDK handle; cleanup may conservatively call Stop after a
        // failed transition because the vendor does not define the partial state.
        status_result(unsafe { bindings::MV3D_LP_StopMeasure(handle.as_ptr()) })
    }

    pub(crate) fn soft_trigger(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device exclusively owns this live SDK handle; trigger mode and call order are
        // validated by the SDK.
        status_result(unsafe { bindings::MV3D_LP_SoftTrigger(handle.as_ptr()) })
    }

    pub(crate) fn clear_buffer(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device owns the handle; the safe facade exposes only owned copies of SDK buffers.
        status_result(unsafe { bindings::MV3D_LP_ClearDataBuffer(handle.as_ptr()) })
    }

    pub(crate) fn get_image(&self, handle: Handle, timeout_ms: u32) -> DriverResult<Image> {
        let mut image = zeroed_image();
        // SAFETY: image is a fully zeroed writable SDK output, Device owns the live handle, and
        // Device's unique ownership prevents another safe call from using this handle until the
        // descriptor and payload copies below finish.
        let status = unsafe { bindings::MV3D_LP_GetImage(handle.as_ptr(), &mut image, timeout_ms) };
        status_result(status)?;
        // SAFETY: On success the audited SDK contract guarantees that every non-null output
        // pointer remains readable for its reported extent until the immediate copy completes.
        unsafe { image_from_native(&image, LengthRule::Padded) }
    }

    pub(crate) fn register_image_callback(
        &self,
        handle: Handle,
        cookie: CallbackCookie,
    ) -> DriverResult<()> {
        // SAFETY: the callback function has the SDK's system calling convention and static
        // lifetime. The opaque cookie is never dereferenced and is not reused by the registry.
        status_result(unsafe {
            bindings::MV3D_LP_RegisterImageDataCallBack(
                handle.as_ptr(),
                Some(image_trampoline),
                cookie.as_user_pointer(),
            )
        })
    }

    pub(crate) fn register_exception_callback(
        &self,
        handle: Handle,
        cookie: CallbackCookie,
    ) -> DriverResult<()> {
        // SAFETY: the same static-trampoline and opaque-cookie guarantees as the image callback
        // apply. Device owns the live handle for this serialized registration call.
        status_result(unsafe {
            bindings::MV3D_LP_RegisterExceptionCallBack(
                handle.as_ptr(),
                Some(exception_trampoline),
                cookie.as_user_pointer(),
            )
        })
    }

    pub(crate) fn get_parameter(&self, handle: Handle, key: &CStr) -> DriverResult<Parameter> {
        let mut parameter = zeroed_parameter();
        // SAFETY: parameter is a fully zeroed writable output and key is NUL-terminated for the
        // call. The tagged union is read only after a successful status and discriminator check.
        status_result(unsafe {
            bindings::MV3D_LP_GetParam(handle.as_ptr(), key.as_ptr(), &mut parameter)
        })?;
        parameter_from_native(&parameter)
    }

    pub(crate) fn set_parameter(
        &self,
        handle: Handle,
        key: &CStr,
        value: &ParameterValue,
    ) -> DriverResult<()> {
        let mut parameter = parameter_to_native(value)?;
        // SAFETY: key is NUL-terminated, parameter's active union member matches its
        // discriminator, and all inactive/reserved storage started zeroed.
        status_result(unsafe {
            bindings::MV3D_LP_SetParam(handle.as_ptr(), key.as_ptr(), &mut parameter)
        })
    }

    pub(crate) fn execute(&self, handle: Handle, key: &CStr) -> DriverResult<()> {
        // SAFETY: Device owns this live handle and key is NUL-terminated for the call.
        status_result(unsafe { bindings::MV3D_LP_Execute(handle.as_ptr(), key.as_ptr()) })
    }

    pub(crate) fn file_access_read(
        &self,
        handle: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        let mut access = bindings::MV3D_LP_FILE_ACCESS {
            pUserFileName: user_file_name.as_ptr(),
            pDevFileName: device_file_name.as_ptr(),
            nReserved: [0; 32],
        };
        // SAFETY: Device owns the handle, the `[IN]` descriptor is initialized for this call, and
        // both strings are NUL-terminated for the duration of the call.
        status_result(unsafe { bindings::MV3D_LP_FileAccessRead(handle.as_ptr(), &mut access) })
    }

    pub(crate) fn file_access_write(
        &self,
        handle: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        let mut access = bindings::MV3D_LP_FILE_ACCESS {
            pUserFileName: user_file_name.as_ptr(),
            pDevFileName: device_file_name.as_ptr(),
            nReserved: [0; 32],
        };
        // SAFETY: the same initialized descriptor and live handle guarantees as FileAccessRead apply.
        status_result(unsafe { bindings::MV3D_LP_FileAccessWrite(handle.as_ptr(), &mut access) })
    }

    pub(crate) fn file_access_progress(&self, handle: Handle) -> DriverResult<FileProgress> {
        let mut progress = bindings::MV3D_LP_FILE_ACCESS_PROGRESS {
            nCompleted: 0,
            nTotal: 0,
            nReserved: [0; 32],
        };
        // SAFETY: Device owns the live handle and progress is a fully initialized writable output.
        status_result(unsafe {
            bindings::MV3D_LP_GetFileAccessProgress(handle.as_ptr(), &mut progress)
        })?;
        Ok(FileProgress {
            completed: progress.nCompleted,
            total: progress.nTotal,
        })
    }

    pub(crate) fn map_depth_to_point_cloud(&self, input: ImageRef<'_>) -> DriverResult<Image> {
        let mut input = image_input_to_native(input)?;
        let mut output = zeroed_image();
        // SAFETY: input borrows a validated payload for the duration of this serialized call;
        // the vendor marks it [IN], so the SDK must not write through its legacy mutable pointer.
        status_result(unsafe { bindings::MV3D_LP_MapDepthToPointCloud(&mut input, &mut output) })?;
        // SAFETY: the SDK reported success, so the output descriptor is initialized and its
        // buffers stay valid until the next image-processing call on this serialized session.
        unsafe { processed_image_from_native(&output, ImageType::POINT_CLOUD) }
    }

    pub(crate) fn map_depth_to_point_cloud_round(
        &self,
        inputs: &[ImageRef<'_>],
    ) -> DriverResult<Image> {
        let mut inputs = prepare_multi_inputs(inputs)?;
        let count = u32::try_from(inputs.len()).map_err(|_| invalid_image_count(inputs.len()))?;
        let mut output = zeroed_image();
        // SAFETY: `inputs` holds `count` validated descriptors borrowing live [IN] payloads for
        // this serialized call; `output` is an initialized descriptor the SDK writes into.
        status_result(unsafe {
            bindings::MV3D_LP_MapDepthToPointCloudRound(inputs.as_mut_ptr(), count, &mut output)
        })?;
        // SAFETY: the SDK reported success, so the output descriptor is initialized and its
        // buffers stay valid until the next image-processing call on this serialized session.
        unsafe { processed_image_from_native(&output, ImageType::POINT_CLOUD) }
    }

    pub(crate) fn convert_image(
        &self,
        input: ImageRef<'_>,
        target: ImageType,
    ) -> DriverResult<Image> {
        let mut input = image_input_to_native(input)?;
        let mut output = zeroed_image();
        output.enImageType = target.raw();
        // SAFETY: `input` borrows a validated [IN] payload for this serialized call; `output`
        // carries only the requested target type and is written by the SDK.
        status_result(unsafe { bindings::MV3D_LP_ImageConvert(&mut input, &mut output) })?;
        // SAFETY: the SDK reported success, so the output descriptor is initialized and its
        // buffers stay valid until the next image-processing call on this serialized session.
        unsafe { processed_image_from_native(&output, target) }
    }

    pub(crate) fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> DriverResult<Image> {
        let mut inputs = prepare_multi_inputs(inputs)?;
        let count = u32::try_from(inputs.len()).map_err(|_| invalid_image_count(inputs.len()))?;
        let mut output = zeroed_image();
        // SAFETY: `inputs` holds `count` validated descriptors borrowing live [IN] payloads for
        // this serialized call; `output` is an initialized descriptor the SDK writes into.
        status_result(unsafe {
            bindings::MV3D_LP_DepthMosaic(inputs.as_mut_ptr(), count, &mut output)
        })?;
        // SAFETY: the SDK reported success, so the output descriptor is initialized and its
        // buffers stay valid until the next image-processing call on this serialized session.
        unsafe { processed_image_from_native(&output, ImageType::DEPTH) }
    }

    pub(crate) fn save_image(
        &self,
        input: ImageRef<'_>,
        format: ImageFileFormat,
        file_name: &CStr,
    ) -> DriverResult<()> {
        let mut input = image_input_to_native(input)?;
        // SAFETY: `input` borrows a validated [IN] payload and `file_name` is a NUL-terminated
        // C string; both stay live for this synchronous call.
        status_result(unsafe {
            bindings::MV3D_LP_SaveImage(&mut input, format as i32, file_name.as_ptr())
        })
    }

    #[cfg(feature = "display-windows")]
    pub(crate) fn display_image(
        &self,
        input: ImageRef<'_>,
        window: NonZeroIsize,
        range: DisplayRange,
    ) -> DriverResult<()> {
        let mut input = image_input_to_native(input)?;
        let (display_type, minimum, maximum) = match range {
            DisplayRange::Auto => (bindings::DisplayType_Auto, 0, 0),
            DisplayRange::Manual { minimum, maximum } => {
                (bindings::DisplayType_Manual, minimum, maximum)
            }
        };
        // SAFETY: `input` was validated above and borrows live vendor-[IN] payloads; `window` came
        // from a borrowed Win32 raw-window-handle. Both remain live for this synchronous call.
        unsafe {
            native_display_image_call(
                &mut input,
                window.get() as *mut std::ffi::c_void,
                display_type,
                minimum,
                maximum,
            )
        }
    }
}

#[cfg(not(native_sdk))]
// Default builds type-check the safe API without referencing the vendor import library.
macro_rules! unavailable_methods {
    ($(fn $name:ident($($argument:ident: $argument_type:ty),*) -> $output:ty;)+) => {
        impl NativeDriver {
            $(
                pub(crate) fn $name(&self, $($argument: $argument_type),*) -> DriverResult<$output> {
                    let _ = ($($argument),*);
                    unreachable!("NativeDriver is constructed only with native support")
                }
            )+
        }
    };
}

#[cfg(not(native_sdk))]
unavailable_methods! {
    fn finalize() -> ();
    fn device_number() -> u32;
    fn device_list(capacity: usize) -> Vec<DeviceInfo>;
    fn set_ip_config(serial: &CStr, config: &IpConfigRaw) -> ();
    fn open_by_ip(ip: &CStr) -> Handle;
    fn open_by_serial(serial: &CStr) -> Handle;
    fn close(handle: Handle) -> ();
    fn start(handle: Handle) -> ();
    fn stop(handle: Handle) -> ();
    fn soft_trigger(handle: Handle) -> ();
    fn clear_buffer(handle: Handle) -> ();
    fn get_image(handle: Handle, timeout_ms: u32) -> Image;
    fn register_image_callback(handle: Handle, cookie: CallbackCookie) -> ();
    fn register_exception_callback(handle: Handle, cookie: CallbackCookie) -> ();
    fn get_parameter(handle: Handle, key: &CStr) -> Parameter;
    fn set_parameter(handle: Handle, key: &CStr, value: &ParameterValue) -> ();
    fn execute(handle: Handle, key: &CStr) -> ();
    fn file_access_read(handle: Handle, user_file_name: &CStr, device_file_name: &CStr) -> ();
    fn file_access_write(handle: Handle, user_file_name: &CStr, device_file_name: &CStr) -> ();
    fn file_access_progress(handle: Handle) -> FileProgress;
    fn map_depth_to_point_cloud(input: ImageRef<'_>) -> Image;
    fn map_depth_to_point_cloud_round(inputs: &[ImageRef<'_>]) -> Image;
    fn convert_image(input: ImageRef<'_>, target: ImageType) -> Image;
    fn mosaic_depth(inputs: &[ImageRef<'_>]) -> Image;
    fn save_image(input: ImageRef<'_>, format: ImageFileFormat, file_name: &CStr) -> ();
}

#[cfg(any(test, native_sdk))]
fn image_input_to_native(input: ImageRef<'_>) -> DriverResult<bindings::MV3D_LP_IMAGE_DATA> {
    let data_len = u32::try_from(input.data.len())
        .map_err(|_| input_too_long("image data", u32::MAX as usize, input.data.len()))?;
    let intensity_len = match input.intensity_data {
        Some(intensity) => u32::try_from(intensity.len()).map_err(|_| {
            input_too_long("image intensity data", u32::MAX as usize, intensity.len())
        })?,
        None => 0,
    };
    let native = bindings::MV3D_LP_IMAGE_DATA {
        enImageType: input.image_type.raw(),
        nWidth: input.width,
        nHeight: input.height,
        pData: input.data.as_ptr().cast_mut(),
        nDataLen: data_len,
        pIntensityData: input
            .intensity_data
            .map_or(ptr::null_mut(), |bytes| bytes.as_ptr().cast_mut()),
        nIntensityDataLen: intensity_len,
        nFrameNum: input.frame_number,
        nTimeStamp: input.device_timestamp,
        bValid: i32::from(input.valid),
        fXScale: input.calibration.x_scale,
        fYScale: input.calibration.y_scale,
        fZScale: input.calibration.z_scale,
        nXOffset: input.calibration.x_offset,
        nYOffset: input.calibration.y_offset,
        nZOffset: input.calibration.z_offset,
        pExposureTimeStamp: input
            .exposure_timestamps
            .map_or(ptr::null_mut(), |timestamps| timestamps.as_ptr().cast_mut()),
        nReserved: [0; 12],
    };
    if let Some(timestamps) = input.exposure_timestamps {
        let height = usize::try_from(input.height).unwrap_or(usize::MAX);
        if timestamps.len() != height {
            return Err(invalid_image_layout("exposure timestamp count"));
        }
    }
    validate_image_layout(&native, LengthRule::Exact).map_err(|error| match error {
        DriverError::Contract(ContractViolation::InvalidValue { field })
        | DriverError::Contract(ContractViolation::LengthMismatch { field, .. })
        | DriverError::Contract(ContractViolation::LengthOverflow { field })
        | DriverError::Contract(ContractViolation::NullPointerWithLength { field, .. }) => {
            invalid_image_layout(field)
        }
        other => other,
    })?;
    Ok(native)
}

#[cfg(all(native_sdk, feature = "display-windows"))]
/// Calls the raw display entry point with an initialized descriptor and borrowed window handle.
///
/// # Safety
///
/// `image` must point to an initialized descriptor whose borrowed payloads remain readable for
/// the call, and `window` must be a live Win32 `HWND` accepted by the vendor runtime.
unsafe fn native_display_image_call(
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    window: *mut std::ffi::c_void,
    display_type: bindings::Mv3dLpDisplayType,
    minimum: i32,
    maximum: i32,
) -> DriverResult<()> {
    // SAFETY: The only caller is NativeDriver::display_image, which validated the descriptor and
    // borrowed the HWND for this synchronous call.
    status_result(unsafe {
        bindings::MV3D_LP_DisplayImage(image, window, display_type, minimum, maximum)
    })
}

#[cfg(native_sdk)]
fn prepare_multi_inputs(
    inputs: &[ImageRef<'_>],
) -> DriverResult<Vec<bindings::MV3D_LP_IMAGE_DATA>> {
    if inputs.len() > MAX_MULTI_IMAGE_COUNT {
        return Err(invalid_image_count(inputs.len()));
    }
    inputs.iter().copied().map(image_input_to_native).collect()
}

#[cfg(native_sdk)]
unsafe fn processed_image_from_native(
    output: &bindings::MV3D_LP_IMAGE_DATA,
    expected: ImageType,
) -> DriverResult<Image> {
    if output.enImageType != expected.raw() {
        return Err(invalid_sdk_image_value("output image type"));
    }
    // SAFETY: the caller holds the session image-processing lock and guarantees a successful SDK
    // ImgProc call. Exact packed sizes are required for processed output.
    unsafe { image_from_native(output, LengthRule::Exact) }
}

#[cfg(native_sdk)]
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
enum LengthRule {
    /// GetImage / callback: known formats may include padding.
    Padded,
    /// User input and processed SDK output: known formats must match packed size.
    Exact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ValidatedImageLayout {
    data_len: usize,
    intensity_len: Option<usize>,
    exposure_count: Option<usize>,
    exposure_bytes: usize,
}

fn length_ok(actual: usize, expected: usize, rule: LengthRule) -> bool {
    match rule {
        LengthRule::Padded => actual >= expected,
        LengthRule::Exact => actual == expected,
    }
}

fn validate_image_layout(
    image: &bindings::MV3D_LP_IMAGE_DATA,
    rule: LengthRule,
) -> DriverResult<ValidatedImageLayout> {
    if image.nWidth == 0 {
        return Err(invalid_sdk_image_value("width"));
    }
    if image.nHeight == 0 {
        return Err(invalid_sdk_image_value("height"));
    }

    let width = usize_from_u32(image.nWidth, "dimensions")?;
    let height = usize_from_u32(image.nHeight, "dimensions")?;
    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| sdk_length_overflow("dimensions"))?;
    let data_len = usize_from_u32(image.nDataLen, "data")?;

    if data_len != 0 && image.pData.is_null() {
        return Err(sdk_null_pointer_with_length("data", data_len));
    }
    if data_len == 0 {
        return Err(invalid_sdk_image_value("data length"));
    }

    if let Some(bytes_per_pixel) = known_bytes_per_pixel(image.enImageType) {
        let expected = pixels
            .checked_mul(bytes_per_pixel)
            .ok_or_else(|| sdk_length_overflow("data"))?;
        if !length_ok(data_len, expected, rule) {
            return Err(sdk_length_mismatch("data", expected, data_len));
        }
    }

    let intensity_len = usize_from_u32(image.nIntensityDataLen, "intensity data")?;
    let intensity_len = match intensity_len {
        0 => None,
        length if image.pIntensityData.is_null() => {
            return Err(sdk_null_pointer_with_length("intensity data", length));
        }
        length => {
            if !length_ok(length, pixels, rule) {
                return Err(sdk_length_mismatch("intensity data", pixels, length));
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
            .ok_or_else(|| sdk_length_overflow("exposure timestamps"))?,
        None => 0,
    };

    data_len
        .checked_add(intensity_len.unwrap_or(0))
        .and_then(|bytes| bytes.checked_add(exposure_bytes))
        .ok_or_else(|| sdk_length_overflow("frame payloads"))?;

    Ok(ValidatedImageLayout {
        data_len,
        intensity_len,
        exposure_count,
        exposure_bytes,
    })
}

unsafe fn image_from_native(
    image: &bindings::MV3D_LP_IMAGE_DATA,
    rule: LengthRule,
) -> DriverResult<Image> {
    let layout = validate_image_layout(image, rule)?;

    let mut data = Vec::with_capacity(layout.data_len);
    let mut intensity_data = layout.intensity_len.map(Vec::with_capacity);
    let mut exposure_timestamps = layout.exposure_count.map(Vec::with_capacity);

    // SAFETY: validate_image_layout established a non-null data pointer and checked length.
    let source = unsafe { std::slice::from_raw_parts(image.pData.cast_const(), layout.data_len) };
    data.extend_from_slice(source);

    if let (Some(destination), Some(length)) = (&mut intensity_data, layout.intensity_len) {
        // SAFETY: validate_image_layout checked the optional intensity pointer and length.
        let source =
            unsafe { std::slice::from_raw_parts(image.pIntensityData.cast_const(), length) };
        destination.extend_from_slice(source);
    }

    if let (Some(destination), Some(count)) = (&mut exposure_timestamps, layout.exposure_count) {
        // SAFETY: validation bounded `height * sizeof(i64)` readable bytes.
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

    Ok(Image {
        image_type: ImageType::from_raw(image.enImageType),
        width: image.nWidth,
        height: image.nHeight,
        data,
        intensity_data,
        exposure_timestamps,
        frame_number: image.nFrameNum,
        device_timestamp: image.nTimeStamp,
        valid: image.bValid != 0,
        calibration: ImageCalibration {
            x_scale: image.fXScale,
            y_scale: image.fYScale,
            z_scale: image.fZScale,
            x_offset: image.nXOffset,
            y_offset: image.nYOffset,
            z_offset: image.nZOffset,
        },
    })
}

pub(crate) unsafe fn callback_image_from_native(
    image: &bindings::MV3D_LP_IMAGE_DATA,
) -> DriverResult<Image> {
    // SAFETY: the trampoline keeps the descriptor and payloads readable for this copy.
    unsafe { image_from_native(image, LengthRule::Padded) }
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

fn usize_from_u32(value: u32, field: &'static str) -> DriverResult<usize> {
    usize::try_from(value).map_err(|_| sdk_length_overflow(field))
}

fn invalid_input(field: &'static str, violation: InputViolation) -> DriverError {
    DriverError::InvalidInput { field, violation }
}

fn invalid_image_count(actual: usize) -> DriverError {
    invalid_input(
        "images",
        InputViolation::ImageCount {
            minimum: 0,
            maximum: MAX_MULTI_IMAGE_COUNT,
            actual,
        },
    )
}

fn invalid_image_layout(field: &'static str) -> DriverError {
    invalid_input("image", InputViolation::InvalidImageLayout { field })
}

fn input_too_long(field: &'static str, maximum: usize, actual: usize) -> DriverError {
    invalid_input(
        field,
        InputViolation::TooLong {
            max: maximum,
            actual,
        },
    )
}

fn invalid_sdk_image_value(field: &'static str) -> DriverError {
    DriverError::Contract(ContractViolation::InvalidValue { field })
}

fn sdk_null_pointer_with_length(field: &'static str, length: usize) -> DriverError {
    DriverError::Contract(ContractViolation::NullPointerWithLength { field, length })
}

fn sdk_length_mismatch(field: &'static str, expected: usize, actual: usize) -> DriverError {
    DriverError::Contract(ContractViolation::LengthMismatch {
        field,
        expected,
        actual,
    })
}

fn sdk_length_overflow(field: &'static str) -> DriverError {
    DriverError::Contract(ContractViolation::LengthOverflow { field })
}

pub(crate) fn zeroed_parameter() -> bindings::MV3D_LP_PARAM {
    // SAFETY: The C tagged union and its containing integer/byte fields admit an all-zero bit
    // pattern. Zeroing the entire object also satisfies the SDK reserved-byte contract.
    unsafe { MaybeUninit::zeroed().assume_init() }
}

#[cfg(native_sdk)]
/// Copies one fixed native device descriptor directly into its owned Rust record.
fn device_info_from_native(native: bindings::MV3D_LP_DEVICE_INFO) -> DeviceInfo {
    DeviceInfo {
        manufacturer_name: SdkText::from_sdk_bytes(bounded_c_bytes(&native.chManufacturerName)),
        model_name: SdkText::from_sdk_bytes(bounded_c_bytes(&native.chModelName)),
        device_version: SdkText::from_sdk_bytes(bounded_c_bytes(&native.chDeviceVersion)),
        manufacturer_specific_info: SdkText::from_sdk_bytes(bounded_c_bytes(
            &native.chManufacturerSpecificInfo,
        )),
        serial_number: SerialNumber::from_sdk_bytes(bounded_c_bytes(&native.chSerialNumber)),
        user_defined_name: SdkText::from_sdk_bytes(bounded_c_bytes(&native.chUserDefinedName)),
        mac_address: native.chMacAddress,
        ip_configuration_mode: crate::device::IpConfigurationMode::from_raw(native.enIPCfgMode),
        current_ip: parse_optional_ipv4(&bounded_c_bytes(&native.chCurrentIp)),
        current_subnet_mask: parse_optional_ipv4(&bounded_c_bytes(&native.chCurrentSubNetMask)),
        default_gateway: parse_optional_ipv4(&bounded_c_bytes(&native.chDefultGateWay)),
        network_interface_ip: parse_optional_ipv4(&bounded_c_bytes(&native.chNetExport)),
        device_type_info: native.nDevTypeInfo,
    }
}

pub(crate) fn parameter_from_native(
    parameter: &bindings::MV3D_LP_PARAM,
) -> DriverResult<Parameter> {
    match parameter.enParamType {
        bindings::ParamType_Bool => {
            // SAFETY: enParamType identifies bBoolParam as the active union member.
            let value = unsafe { parameter.ParamInfo.bBoolParam };
            Ok(Parameter::Bool(value != 0))
        }
        bindings::ParamType_Int => {
            // SAFETY: enParamType identifies stIntParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stIntParam };
            Ok(Parameter::Integer {
                value: value.nCurValue,
                min: value.nMin,
                max: value.nMax,
                increment: value.nInc,
            })
        }
        bindings::ParamType_Float => {
            // SAFETY: enParamType identifies stFloatParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stFloatParam };
            Ok(Parameter::Float {
                value: value.fCurValue,
                min: value.fMin,
                max: value.fMax,
            })
        }
        bindings::ParamType_Enum => {
            // SAFETY: enParamType identifies stEnumParam as the active union member.
            let value = unsafe { parameter.ParamInfo.stEnumParam };
            let supported_count = usize::try_from(value.nSupportedNum).unwrap_or(usize::MAX);
            if supported_count > bindings::MV3D_LP_MAX_ENUM_COUNT {
                return Err(DriverError::Contract(
                    ContractViolation::CountExceedsCapacity {
                        field: "supported enumeration values",
                        count: supported_count,
                        capacity: bindings::MV3D_LP_MAX_ENUM_COUNT,
                    },
                ));
            }
            Ok(Parameter::Enumeration {
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
                return Err(DriverError::Contract(ContractViolation::OutputTooLarge {
                    field: "parameter string maximum length",
                    limit: bindings::MV3D_LP_MAX_STRING_LENGTH,
                    actual: usize::try_from(value.nMaxLength).unwrap_or(usize::MAX),
                }));
            }
            Ok(Parameter::String {
                value: SdkText::from_sdk_bytes(bounded_c_bytes(&value.chCurValue)),
                max_length: value.nMaxLength,
            })
        }
        other => Err(DriverError::Contract(
            ContractViolation::UnknownDiscriminant {
                field: "parameter type",
                raw: other as u32,
            },
        )),
    }
}

pub(crate) fn parameter_to_native(value: &ParameterValue) -> DriverResult<bindings::MV3D_LP_PARAM> {
    let mut parameter = zeroed_parameter();
    match value {
        ParameterValue::Bool(value) => {
            parameter.enParamType = bindings::ParamType_Bool;
            parameter.ParamInfo.bBoolParam = i32::from(*value);
        }
        ParameterValue::Integer(value) => {
            parameter.enParamType = bindings::ParamType_Int;
            parameter.ParamInfo.stIntParam = bindings::MV3D_LP_INTPARAM {
                nCurValue: *value,
                nMax: 0,
                nMin: 0,
                nInc: 0,
            };
        }
        ParameterValue::Float(value) => {
            parameter.enParamType = bindings::ParamType_Float;
            parameter.ParamInfo.stFloatParam = bindings::MV3D_LP_FLOATPARAM {
                fCurValue: *value,
                fMax: 0.0,
                fMin: 0.0,
            };
        }
        ParameterValue::Enumeration(value) => {
            parameter.enParamType = bindings::ParamType_Enum;
            parameter.ParamInfo.stEnumParam = bindings::MV3D_LP_ENUMPARAM {
                nCurValue: *value,
                nSupportedNum: 0,
                nSupportValue: [0; bindings::MV3D_LP_MAX_ENUM_COUNT],
            };
        }
        ParameterValue::String(value) => {
            let bytes = value.as_bytes();
            if bytes.len() >= bindings::MV3D_LP_MAX_STRING_LENGTH {
                return Err(invalid_input(
                    "parameter value",
                    InputViolation::TooLong {
                        max: bindings::MV3D_LP_MAX_STRING_LENGTH - 1,
                        actual: bytes.len(),
                    },
                ));
            }
            let mut string = bindings::MV3D_LP_STRINGPARAM {
                chCurValue: [0; bindings::MV3D_LP_MAX_STRING_LENGTH],
                nMaxLength: 0,
            };
            for (destination, source) in string.chCurValue.iter_mut().zip(bytes) {
                *destination = *source as i8;
            }
            parameter.enParamType = bindings::ParamType_String;
            parameter.ParamInfo.stStringParam = string;
        }
    }
    Ok(parameter)
}

/// Copies one fixed C buffer through its first NUL byte.
pub(crate) fn bounded_c_bytes<const N: usize>(source: &[i8; N]) -> Vec<u8> {
    let length = source.iter().position(|byte| *byte == 0).unwrap_or(N);
    source[..length].iter().map(|byte| *byte as u8).collect()
}

#[cfg(native_sdk)]
fn as_c_char_array<const N: usize>(source: &[u8; N]) -> [i8; N] {
    std::array::from_fn(|index| source[index] as i8)
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::{
        LengthRule, image_from_native, image_input_to_native, parameter_from_native,
        parameter_to_native, zeroed_image, zeroed_parameter,
    };
    use crate::bindings;
    use crate::driver::DriverError;
    use crate::error::{ContractViolation, InputViolation};
    use crate::frame::{ImageCalibration, ImageRef, ImageType};
    use crate::parameter::{Parameter, ParameterValue};
    use crate::text::SdkText;

    // 验证 SDK 图像的指针/长度边界，并确认 callback 返回前完成深拷贝。
    #[test]
    fn image_payloads_are_validated_and_owned() {
        let mut data = [1_u8, 2];
        let mut intensity = [3_u8, 4];
        let mut exposure = [5_i64];
        let mut image = zeroed_image();
        image.enImageType = bindings::ImageType_Mono8;
        image.nWidth = 2;
        image.nHeight = 1;
        image.pData = data.as_mut_ptr();
        image.nDataLen = data.len() as u32;
        image.pIntensityData = intensity.as_mut_ptr();
        image.nIntensityDataLen = intensity.len() as u32;
        image.pExposureTimeStamp = exposure.as_mut_ptr();

        // SAFETY: all descriptor pointers refer to the live arrays above for their declared sizes.
        let frame = unsafe { image_from_native(&image, LengthRule::Padded) }.unwrap();
        data.fill(9);
        intensity.fill(9);
        exposure.fill(9);
        assert_eq!(frame.data, [1, 2]);
        assert_eq!(frame.intensity_data.as_deref(), Some([3, 4].as_slice()));
        assert_eq!(frame.exposure_timestamps.as_deref(), Some([5].as_slice()));

        image.pData = ptr::null_mut();
        assert!(matches!(
            // SAFETY: validation rejects the null pointer before reading any payload.
            unsafe { image_from_native(&image, LengthRule::Padded) },
            Err(DriverError::Contract(
                ContractViolation::NullPointerWithLength {
                    field: "data",
                    length: 2
                }
            ))
        ));
    }

    // 验证已知格式及可选平面的布局必须与宽高精确对应。
    #[test]
    fn image_inputs_require_exact_known_layouts() {
        let data = [0; 4];
        let intensity = [0; 4];
        let timestamps = [0; 2];
        let input = ImageRef {
            image_type: ImageType::from_raw(bindings::ImageType_Mono8),
            width: 2,
            height: 2,
            data: &data,
            intensity_data: Some(&intensity),
            exposure_timestamps: Some(&timestamps),
            frame_number: 0,
            device_timestamp: 0,
            valid: true,
            calibration: ImageCalibration::default(),
        };
        assert!(image_input_to_native(input).is_ok());

        let short_data = [0; 3];
        let long_data = [0; 5];
        let short_intensity = [0; 3];
        let long_intensity = [0; 5];
        let short_timestamps = [0; 1];
        let long_timestamps = [0; 3];
        for invalid in [
            ImageRef {
                data: &short_data,
                ..input
            },
            ImageRef {
                data: &long_data,
                ..input
            },
            ImageRef {
                intensity_data: Some(&short_intensity),
                ..input
            },
            ImageRef {
                intensity_data: Some(&long_intensity),
                ..input
            },
            ImageRef {
                exposure_timestamps: Some(&short_timestamps),
                ..input
            },
            ImageRef {
                exposure_timestamps: Some(&long_timestamps),
                ..input
            },
        ] {
            assert!(matches!(
                image_input_to_native(invalid),
                Err(DriverError::InvalidInput {
                    violation: InputViolation::InvalidImageLayout { .. },
                    ..
                })
            ));
        }
    }

    // 验证 parameter union 仅按 discriminator 读取，并限制厂商返回的 enum 数量。
    #[test]
    fn parameter_union_uses_tag_and_checks_enum_count() {
        let mut parameter = zeroed_parameter();
        parameter.enParamType = bindings::ParamType_Enum;
        parameter.ParamInfo.stEnumParam = bindings::MV3D_LP_ENUMPARAM {
            nCurValue: 7,
            nSupportedNum: 2,
            nSupportValue: [11, 13, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        };
        assert_eq!(
            parameter_from_native(&parameter).unwrap(),
            Parameter::Enumeration {
                value: 7,
                supported: vec![11, 13]
            }
        );

        parameter.ParamInfo.stEnumParam.nSupportedNum =
            (bindings::MV3D_LP_MAX_ENUM_COUNT + 1) as u32;
        assert!(matches!(
            parameter_from_native(&parameter),
            Err(DriverError::Contract(
                ContractViolation::CountExceedsCapacity { .. }
            ))
        ));

        let boolean = parameter_to_native(&ParameterValue::Bool(true)).unwrap();
        assert_eq!(boolean.enParamType, bindings::ParamType_Bool);
        // SAFETY: the discriminator above identifies bBoolParam as the active member.
        assert_eq!(unsafe { boolean.ParamInfo.bBoolParam }, 1);

        assert!(matches!(
            SdkText::new(b"a\0b"),
            Err(crate::error::Error::InvalidInput {
                violation: InputViolation::InteriorNul,
                ..
            })
        ));
    }
}
