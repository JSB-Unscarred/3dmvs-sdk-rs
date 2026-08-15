#![cfg_attr(not(feature = "native"), allow(dead_code, unused_imports))]

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
use std::ffi::CStr;
use std::mem::{MaybeUninit, size_of};
#[cfg(all(
    feature = "display-windows",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
use std::num::NonZeroIsize;
#[cfg(any(
    test,
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
use std::ptr;

use crate::bindings;
#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
use crate::callback::{CallbackCookie, exception_trampoline, image_trampoline};
#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
use crate::device::{DeviceInfoRaw, DeviceListAttempt, IpConfigRaw};
#[cfg(all(
    feature = "display-windows",
    target_os = "windows",
    target_arch = "x86_64",
    target_env = "msvc"
))]
use crate::display::DisplayRangeRecord;
#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
use crate::driver::{Driver, Handle, status_result};
use crate::driver::{DriverError, DriverResult};
use crate::error::{ContractViolation, InputViolation};
use crate::file_transfer::FileProgress;
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::parameter::{ParameterRecord, ParameterValueRecord};

const MAX_MULTI_IMAGE_COUNT: usize = 8;

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
pub(crate) struct NativeDriver;

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
impl Driver for NativeDriver {
    fn version(&self) -> DriverResult<Vec<u8>> {
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

    fn initialize(&self) -> DriverResult<()> {
        // SAFETY: Runtime admits Initialize only once during the process lifetime.
        status_result(unsafe { bindings::MV3D_LP_Initialize() })
    }

    fn finalize(&self) -> DriverResult<()> {
        // SAFETY: Runtime consumes the sole session owner before calling Finalize once.
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

    fn open_by_ip(&self, ip: &CStr) -> DriverResult<Handle> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a valid writable handle slot and ip is NUL-terminated for the call.
        let status = unsafe { bindings::MV3D_LP_OpenDeviceByIP(&mut raw, ip.as_ptr()) };
        status_result(status)?;
        Handle::from_ptr(raw).ok_or(DriverError::Contract(ContractViolation::NullPointer {
            field: "device handle",
        }))
    }

    fn open_by_serial(&self, serial: &CStr) -> DriverResult<Handle> {
        let mut raw = ptr::null_mut();
        // SAFETY: raw is a valid writable handle slot and serial is NUL-terminated for the call.
        let status = unsafe { bindings::MV3D_LP_OpenDeviceBySN(&mut raw, serial.as_ptr()) };
        status_result(status)?;
        Handle::from_ptr(raw).ok_or(DriverError::Contract(ContractViolation::NullPointer {
            field: "device handle",
        }))
    }

    fn close(&self, handle: Handle) -> DriverResult<()> {
        let mut raw = handle.as_ptr();
        // SAFETY: handle originated from a successful SDK open call and its Device owner calls
        // CloseDevice at most once. Returning consumes the handle even when status reports an
        // error; the SDK has also quiesced callbacks and released asynchronous input borrows.
        status_result(unsafe { bindings::MV3D_LP_CloseDevice(&mut raw) })
    }

    fn start(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device validates the state and owns this live SDK handle.
        status_result(unsafe { bindings::MV3D_LP_StartMeasure(handle.as_ptr()) })
    }

    fn stop(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device owns this live SDK handle; cleanup may conservatively call Stop after a
        // failed transition because the vendor does not define the partial state.
        status_result(unsafe { bindings::MV3D_LP_StopMeasure(handle.as_ptr()) })
    }

    fn soft_trigger(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device exclusively owns this live SDK handle; trigger mode and call order are
        // validated by the SDK.
        status_result(unsafe { bindings::MV3D_LP_SoftTrigger(handle.as_ptr()) })
    }

    fn clear_buffer(&self, handle: Handle) -> DriverResult<()> {
        // SAFETY: Device owns the handle; the safe facade exposes only owned copies of SDK buffers.
        status_result(unsafe { bindings::MV3D_LP_ClearDataBuffer(handle.as_ptr()) })
    }

    fn get_image(&self, handle: Handle, timeout_ms: u32) -> DriverResult<FrameRecord> {
        let mut image = zeroed_image();
        // SAFETY: image is a fully zeroed writable SDK output, Device owns the live handle, and
        // Device's unique ownership prevents another safe call from using this handle until the
        // descriptor and payload copies below finish.
        let status = unsafe { bindings::MV3D_LP_GetImage(handle.as_ptr(), &mut image, timeout_ms) };
        // A failed SDK call does not initialize a trustworthy output descriptor. In particular,
        // never inspect or dereference pointer fields before checking the status.
        status_result(status)?;
        // SAFETY: On success the audited SDK contract guarantees that every non-null output
        // pointer remains readable for its reported extent until the immediate copy completes.
        // validate_image_layout checks lengths, null pairs, and arithmetic before
        // image_from_native dereferences any pointer.
        unsafe { image_from_native(&image) }
    }

    fn register_image_callback(&self, handle: Handle, cookie: CallbackCookie) -> DriverResult<()> {
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

    fn register_exception_callback(
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
        // SAFETY: Device owns this live handle and key is NUL-terminated for the call.
        status_result(unsafe { bindings::MV3D_LP_Execute(handle.as_ptr(), key.as_ptr()) })
    }

    fn file_access_read(
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
        // both strings are NUL-terminated. Device retains them after a successful async start.
        status_result(unsafe { bindings::MV3D_LP_FileAccessRead(handle.as_ptr(), &mut access) })
    }

    fn file_access_write(
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
        // SAFETY: the same initialized descriptor, retained strings, and live handle guarantees
        // as FileAccessRead apply.
        status_result(unsafe { bindings::MV3D_LP_FileAccessWrite(handle.as_ptr(), &mut access) })
    }

    fn file_access_progress(&self, handle: Handle) -> DriverResult<FileProgress> {
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

    fn map_depth_to_point_cloud(&self, input: ImageInput<'_>) -> DriverResult<FrameRecord> {
        let mut input = image_input_to_native(input)?;
        let mut output = zeroed_image();
        // SAFETY: input borrows a validated payload for the duration of this serialized call;
        // the vendor marks it [IN], so the SDK must not write through its legacy mutable pointer.
        // Output is a fully zeroed writable descriptor and receives transient SDK-owned data.
        status_result(unsafe { bindings::MV3D_LP_MapDepthToPointCloud(&mut input, &mut output) })?;
        // SAFETY: the successful SDK call guarantees its returned payload remains readable until
        // the next image-processing call; Runtime keeps the session image-processing lock through
        // this copy.
        unsafe {
            processed_image_from_native(
                &output,
                ImageTypeRecord::from_raw(bindings::ImageType_PointCloud),
            )
        }
    }

    fn map_depth_to_point_cloud_round(
        &self,
        inputs: &[ImageInput<'_>],
    ) -> DriverResult<FrameRecord> {
        let mut inputs = prepare_multi_inputs(inputs)?;
        let count = u32::try_from(inputs.len()).map_err(|_| invalid_image_count(inputs.len()))?;
        let mut output = zeroed_image();
        // SAFETY: every descriptor borrows a validated, vendor-[IN] read-only payload, count
        // matches the initialized descriptor array, and output is writable for the call.
        status_result(unsafe {
            bindings::MV3D_LP_MapDepthToPointCloudRound(inputs.as_mut_ptr(), count, &mut output)
        })?;
        // SAFETY: see map_depth_to_point_cloud; the output is copied before releasing the gate.
        unsafe {
            processed_image_from_native(
                &output,
                ImageTypeRecord::from_raw(bindings::ImageType_PointCloud),
            )
        }
    }

    fn convert_image(
        &self,
        input: ImageInput<'_>,
        target: ImageTypeRecord,
    ) -> DriverResult<FrameRecord> {
        let mut input = image_input_to_native(input)?;
        let mut output = zeroed_image();
        output.enImageType = target.raw();
        // SAFETY: the header requires only the requested output type in the otherwise zeroed
        // output descriptor. The vendor marks input [IN] and must not write its borrowed payload.
        status_result(unsafe { bindings::MV3D_LP_ImageConvert(&mut input, &mut output) })?;
        // SAFETY: the SDK returned success and its transient output is copied before another call.
        unsafe { processed_image_from_native(&output, target) }
    }

    fn mosaic_depth(&self, inputs: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        let mut inputs = prepare_multi_inputs(inputs)?;
        let count = u32::try_from(inputs.len()).map_err(|_| invalid_image_count(inputs.len()))?;
        let mut output = zeroed_image();
        // SAFETY: descriptors and count describe initialized vendor-[IN] read-only inputs;
        // output is writable and receives SDK-owned transient payloads.
        status_result(unsafe {
            bindings::MV3D_LP_DepthMosaic(inputs.as_mut_ptr(), count, &mut output)
        })?;
        // SAFETY: the SDK output is validated and copied while the process gate is still held.
        unsafe {
            processed_image_from_native(
                &output,
                ImageTypeRecord::from_raw(bindings::ImageType_Depth),
            )
        }
    }

    fn save_image(
        &self,
        input: ImageInput<'_>,
        format: ImageFileFormatRecord,
        file_name: &CStr,
    ) -> DriverResult<()> {
        let mut input = image_input_to_native(input)?;
        // SAFETY: input payload and filename are vendor-[IN], read-only borrows for this
        // synchronous call; the SDK must neither write them nor retain their addresses.
        status_result(unsafe {
            bindings::MV3D_LP_SaveImage(&mut input, format as i32, file_name.as_ptr())
        })
    }

    #[cfg(feature = "display-windows")]
    fn display_image(
        &self,
        input: ImageInput<'_>,
        window: NonZeroIsize,
        range: DisplayRangeRecord,
    ) -> DriverResult<()> {
        let mut input = image_input_to_native(input)?;
        let (display_type, minimum, maximum) = match range {
            DisplayRangeRecord::Auto => (bindings::DisplayType_Auto, 0, 0),
            DisplayRangeRecord::Manual { minimum, maximum } => {
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

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
fn image_input_to_native(input: ImageInput<'_>) -> DriverResult<bindings::MV3D_LP_IMAGE_DATA> {
    if input.width == 0 {
        return Err(invalid_image_layout("width"));
    }
    if input.height == 0 {
        return Err(invalid_image_layout("height"));
    }
    if input.data.is_empty() {
        return Err(invalid_image_layout("data length"));
    }
    let data_len = u32::try_from(input.data.len())
        .map_err(|_| input_too_long("image data", u32::MAX as usize, input.data.len()))?;
    let pixels = usize::try_from(input.width)
        .unwrap_or(usize::MAX)
        .checked_mul(usize::try_from(input.height).unwrap_or(usize::MAX))
        .ok_or_else(|| invalid_image_layout("dimensions"))?;
    if let Some(bytes_per_pixel) = known_bytes_per_pixel(input.image_type.raw()) {
        let expected = pixels
            .checked_mul(bytes_per_pixel)
            .ok_or_else(|| invalid_image_layout("data length"))?;
        if input.data.len() < expected {
            return Err(invalid_image_layout("data length"));
        }
    }
    let intensity_len = match input.intensity_data {
        Some(intensity) if intensity.len() < pixels => {
            return Err(invalid_image_layout("intensity data length"));
        }
        Some(intensity) => u32::try_from(intensity.len()).map_err(|_| {
            input_too_long("image intensity data", u32::MAX as usize, intensity.len())
        })?,
        None => 0,
    };
    if let Some(timestamps) = input.exposure_timestamps {
        if timestamps.len() < usize::try_from(input.height).unwrap_or(usize::MAX) {
            return Err(invalid_image_layout("exposure timestamp count"));
        }
    }
    let exposure_bytes = input.exposure_timestamps.map_or(Ok(0), |timestamps| {
        timestamps
            .len()
            .checked_mul(size_of::<i64>())
            .ok_or_else(|| invalid_image_layout("exposure timestamp bytes"))
    })?;
    input
        .data
        .len()
        .checked_add(usize::try_from(intensity_len).unwrap_or(usize::MAX))
        .and_then(|bytes| bytes.checked_add(exposure_bytes))
        .ok_or_else(|| invalid_image_layout("aggregate input length"))?;

    Ok(bindings::MV3D_LP_IMAGE_DATA {
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
        fXScale: input.x_scale,
        fYScale: input.y_scale,
        fZScale: input.z_scale,
        nXOffset: input.x_offset,
        nYOffset: input.y_offset,
        nZOffset: input.z_offset,
        pExposureTimeStamp: input
            .exposure_timestamps
            .map_or(ptr::null_mut(), |timestamps| timestamps.as_ptr().cast_mut()),
        nReserved: [0; 12],
    })
}

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "display-windows",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
/// Calls the raw display entry point with an initialized descriptor and borrowed window handle.
///
/// # Safety
///
/// `image` must point to an initialized descriptor whose borrowed payloads remain readable for
/// the call, and `window` must be a live Win32 `HWND` accepted by the vendor runtime. Test callers
/// may use dummy values only when the no-native symbol stub is linked.
pub(crate) unsafe fn native_display_image_call(
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    window: *mut std::ffi::c_void,
    display_type: bindings::Mv3dLpDisplayType,
    minimum: i32,
    maximum: i32,
) -> DriverResult<()> {
    // SAFETY: Production callers provide the validated live image and HWND used by
    // NativeDriver. The no-native unit test supplies initialized raw storage and a local symbol
    // stub with the identical C signature.
    status_result(unsafe {
        bindings::MV3D_LP_DisplayImage(image, window, display_type, minimum, maximum)
    })
}

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
fn prepare_multi_inputs(
    inputs: &[ImageInput<'_>],
) -> DriverResult<Vec<bindings::MV3D_LP_IMAGE_DATA>> {
    if inputs.len() > MAX_MULTI_IMAGE_COUNT {
        return Err(invalid_image_count(inputs.len()));
    }
    inputs.iter().copied().map(image_input_to_native).collect()
}

#[cfg(any(
    test,
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
unsafe fn processed_image_from_native(
    output: &bindings::MV3D_LP_IMAGE_DATA,
    expected: ImageTypeRecord,
) -> DriverResult<FrameRecord> {
    if output.enImageType != expected.raw() {
        return Err(invalid_sdk_image_value("output image type"));
    }
    validate_processed_image_lengths(output)?;
    // SAFETY: the caller holds the session image-processing lock and guarantees a successful SDK
    // ImgProc call.
    unsafe { image_from_native(output) }
}

#[cfg(any(
    test,
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
fn validate_processed_image_lengths(image: &bindings::MV3D_LP_IMAGE_DATA) -> DriverResult<()> {
    let pixels = usize::try_from(image.nWidth)
        .unwrap_or(usize::MAX)
        .checked_mul(usize::try_from(image.nHeight).unwrap_or(usize::MAX))
        .ok_or_else(|| sdk_length_overflow("dimensions"))?;
    if let Some(bytes_per_pixel) = known_bytes_per_pixel(image.enImageType) {
        let expected = pixels
            .checked_mul(bytes_per_pixel)
            .ok_or_else(|| sdk_length_overflow("data"))?;
        let actual = usize::try_from(image.nDataLen).unwrap_or(usize::MAX);
        if actual != expected {
            return Err(sdk_length_mismatch("data", expected, actual));
        }
    }
    if image.nIntensityDataLen != 0 {
        let actual = usize::try_from(image.nIntensityDataLen).unwrap_or(usize::MAX);
        if actual != pixels {
            return Err(sdk_length_mismatch("intensity data", pixels, actual));
        }
    }
    Ok(())
}

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
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

    // Pointer/length pairs are checked before format-specific arithmetic so a null pointer with
    // a claimed readable extent is always reported without trying to access it.
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
        if data_len < expected {
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
            if length < pixels {
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

unsafe fn image_from_native(image: &bindings::MV3D_LP_IMAGE_DATA) -> DriverResult<FrameRecord> {
    let layout = validate_image_layout(image)?;

    // Allocate every destination before reading SDK memory. The process terminates on allocation
    // failure, so partially copied native payloads never escape.
    let mut data = Vec::with_capacity(layout.data_len);
    let mut intensity_data = layout.intensity_len.map(Vec::with_capacity);
    let mut exposure_timestamps = layout.exposure_count.map(Vec::with_capacity);

    // SAFETY: validate_image_layout established a non-null data pointer and checked length. The
    // caller of image_from_native guarantees that the SDK allocation is readable for that extent
    // until this immediate copy finishes.
    let source = unsafe { std::slice::from_raw_parts(image.pData.cast_const(), layout.data_len) };
    data.extend_from_slice(source);

    if let (Some(destination), Some(length)) = (&mut intensity_data, layout.intensity_len) {
        // SAFETY: The same conditions as for data apply to the optional intensity pointer, and
        // validate_image_layout additionally checked its minimum pixel count.
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

pub(crate) unsafe fn callback_image_from_native(
    image: &bindings::MV3D_LP_IMAGE_DATA,
) -> DriverResult<FrameRecord> {
    // SAFETY: the native callback trampoline guarantees that `image` and its SDK-owned payloads
    // remain readable for this immediate conversion. The shared converter validates every
    // pointer/length pair and aggregate arithmetic before dereferencing a payload.
    unsafe { image_from_native(image) }
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

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
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
                    ContractViolation::CountExceedsCapacity {
                        field: "supported enumeration values",
                        count: supported_count,
                        capacity: bindings::MV3D_LP_MAX_ENUM_COUNT,
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
                return Err(DriverError::Contract(ContractViolation::OutputTooLarge {
                    field: "parameter string maximum length",
                    limit: bindings::MV3D_LP_MAX_STRING_LENGTH,
                    actual: usize::try_from(value.nMaxLength).unwrap_or(usize::MAX),
                }));
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
            ContractViolation::UnknownDiscriminant {
                field: "parameter type",
                raw: other as u32,
            },
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
                return Err(invalid_input(
                    "parameter value",
                    InputViolation::TooLong {
                        max: bindings::MV3D_LP_MAX_STRING_LENGTH - 1,
                        actual: value.len(),
                    },
                ));
            }
            if value.contains(&0) {
                return Err(invalid_input(
                    "parameter value",
                    InputViolation::InteriorNul,
                ));
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
    Ok(parameter)
}

fn as_u8_array<const N: usize>(source: &[i8; N]) -> [u8; N] {
    std::array::from_fn(|index| source[index] as u8)
}

#[cfg(any(
    all(test, not(miri), not(feature = "native")),
    all(
        feature = "native",
        target_os = "windows",
        target_arch = "x86_64",
        target_env = "msvc"
    )
))]
fn as_c_char_array<const N: usize>(source: &[u8; N]) -> [i8; N] {
    std::array::from_fn(|index| source[index] as i8)
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::{
        image_from_native, parameter_from_native, parameter_to_native, zeroed_image,
        zeroed_parameter,
    };
    use crate::bindings;
    use crate::driver::DriverError;
    use crate::error::{ContractViolation, InputViolation};
    use crate::parameter::{ParameterRecord, ParameterValueRecord};

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
        let frame = unsafe { image_from_native(&image) }.unwrap();
        data.fill(9);
        intensity.fill(9);
        exposure.fill(9);
        assert_eq!(frame.data, [1, 2]);
        assert_eq!(frame.intensity_data.as_deref(), Some([3, 4].as_slice()));
        assert_eq!(frame.exposure_timestamps.as_deref(), Some([5].as_slice()));

        image.pData = ptr::null_mut();
        assert!(matches!(
            // SAFETY: validation rejects the null pointer before reading any payload.
            unsafe { image_from_native(&image) },
            Err(DriverError::Contract(
                ContractViolation::NullPointerWithLength {
                    field: "data",
                    length: 2
                }
            ))
        ));
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
            ParameterRecord::Enumeration {
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

        let boolean = parameter_to_native(&ParameterValueRecord::Bool(true)).unwrap();
        assert_eq!(boolean.enParamType, bindings::ParamType_Bool);
        // SAFETY: the discriminator above identifies bBoolParam as the active member.
        assert_eq!(unsafe { boolean.ParamInfo.bBoolParam }, 1);

        assert!(matches!(
            parameter_to_native(&ParameterValueRecord::String(b"a\0b".to_vec())),
            Err(DriverError::InvalidInput {
                field: "parameter value",
                violation: InputViolation::InteriorNul,
            })
        ));
    }
}
