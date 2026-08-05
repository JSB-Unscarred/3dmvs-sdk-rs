//! No-SDK tests for the production `NativeDriver -> bindings` call path.
//!
//! The exported Rust functions below use the exact C symbol names expected by the bindings. This
//! lets an ordinary unit-test binary exercise production symbol selection and status gates without
//! linking the vendor import library. Miri intentionally excludes this module because it does not
//! use the platform linker's symbol resolution.

use std::cell::RefCell;
use std::collections::HashSet;
use std::ffi::{c_char, c_void};
use std::ptr::{self, NonNull};
use std::sync::Arc;

use crate::bindings;
use crate::callback::{
    CallbackDelivery, CallbackRegistration, ExceptionCallbackSink, FrameCallbackSink,
};
use crate::device::{IpConfigRaw, IpConfiguration};
use crate::driver::{Driver, DriverError, DriverResult, Handle};
use crate::error::{ContractViolation, InvalidInput};
use crate::ffi::{NativeDriver, native_display_image_call, zeroed_image, zeroed_parameter};
use crate::frame::{ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::parameter::ParameterValueRecord;

const INJECTED_STATUS: i32 = 0xE005_0002_u32 as i32;
static UNTERMINATED_VERSION: [u8; 64] = [b'V'; 64];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum RawFfiOp {
    GetVersion,
    Initialize,
    Finalize,
    GetDeviceNumber,
    GetDeviceList,
    SetIpConfig,
    OpenDeviceByIp,
    OpenDeviceBySerial,
    CloseDevice,
    StartMeasure,
    StopMeasure,
    SoftTrigger,
    ClearDataBuffer,
    GetImage,
    RegisterImageCallback,
    RegisterExceptionCallback,
    GetParameter,
    SetParameter,
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

impl RawFfiOp {
    const ALL: &'static [Self] = &[
        Self::GetVersion,
        Self::Initialize,
        Self::Finalize,
        Self::GetDeviceNumber,
        Self::GetDeviceList,
        Self::SetIpConfig,
        Self::OpenDeviceByIp,
        Self::OpenDeviceBySerial,
        Self::CloseDevice,
        Self::StartMeasure,
        Self::StopMeasure,
        Self::SoftTrigger,
        Self::ClearDataBuffer,
        Self::GetImage,
        Self::RegisterImageCallback,
        Self::RegisterExceptionCallback,
        Self::GetParameter,
        Self::SetParameter,
        Self::Execute,
        Self::FileAccessRead,
        Self::FileAccessWrite,
        Self::GetFileAccessProgress,
        Self::MapDepthToPointCloud,
        Self::MapDepthToPointCloudRound,
        Self::ImageConvert,
        Self::DepthMosaic,
        Self::SaveImage,
        Self::DisplayImage,
    ];

    const fn sdk_name(self) -> &'static str {
        match self {
            Self::GetVersion => "MV3D_LP_GetVersion",
            Self::Initialize => "MV3D_LP_Initialize",
            Self::Finalize => "MV3D_LP_Finalize",
            Self::GetDeviceNumber => "MV3D_LP_GetDeviceNumber",
            Self::GetDeviceList => "MV3D_LP_GetDeviceList",
            Self::SetIpConfig => "MV3D_LP_SetIpConfig",
            Self::OpenDeviceByIp => "MV3D_LP_OpenDeviceByIP",
            Self::OpenDeviceBySerial => "MV3D_LP_OpenDeviceBySN",
            Self::CloseDevice => "MV3D_LP_CloseDevice",
            Self::StartMeasure => "MV3D_LP_StartMeasure",
            Self::StopMeasure => "MV3D_LP_StopMeasure",
            Self::SoftTrigger => "MV3D_LP_SoftTrigger",
            Self::ClearDataBuffer => "MV3D_LP_ClearDataBuffer",
            Self::GetImage => "MV3D_LP_GetImage",
            Self::RegisterImageCallback => "MV3D_LP_RegisterImageDataCallBack",
            Self::RegisterExceptionCallback => "MV3D_LP_RegisterExceptionCallBack",
            Self::GetParameter => "MV3D_LP_GetParam",
            Self::SetParameter => "MV3D_LP_SetParam",
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

#[derive(Clone, Copy)]
enum VersionResponse {
    Null,
    Unterminated,
}

struct StubState {
    calls: Vec<RawFfiOp>,
    version: VersionResponse,
    open_returns_handle: bool,
}

impl Default for StubState {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            version: VersionResponse::Null,
            open_returns_handle: false,
        }
    }
}

thread_local! {
    static STUB_STATE: RefCell<StubState> = RefCell::new(StubState::default());
}

fn configure(version: VersionResponse, open_returns_handle: bool) {
    STUB_STATE.with(|state| {
        *state.borrow_mut() = StubState {
            calls: Vec::new(),
            version,
            open_returns_handle,
        };
    });
}

fn record(operation: RawFfiOp) -> i32 {
    STUB_STATE.with(|state| state.borrow_mut().calls.push(operation));
    INJECTED_STATUS
}

fn calls() -> Vec<RawFfiOp> {
    STUB_STATE.with(|state| state.borrow().calls.clone())
}

fn version_response() -> VersionResponse {
    STUB_STATE.with(|state| state.borrow().version)
}

fn open_returns_handle() -> bool {
    STUB_STATE.with(|state| state.borrow().open_returns_handle)
}

fn write_output<T>(output: *mut T, value: T) {
    if !output.is_null() {
        // SAFETY: These stubs are linked only into this crate's no-native unit-test binary. Every
        // call comes from NativeDriver and supplies an initialized writable output slot of `T`.
        unsafe { output.write(value) };
    }
}

fn poisoned_image() -> bindings::MV3D_LP_IMAGE_DATA {
    let mut image = zeroed_image();
    image.enImageType = i32::MAX;
    image.nDataLen = u32::MAX;
    image.nIntensityDataLen = u32::MAX;
    image
}

// 验证 raw FFI 符号清单覆盖所有生产调用，防止新增 native 调用绕过失败测试。
#[test]
fn raw_symbol_ledger_matches_every_production_binding_call() {
    assert_eq!(RawFfiOp::ALL.len(), 28);
    let covered = RawFfiOp::ALL
        .iter()
        .map(|operation| operation.sdk_name())
        .collect::<HashSet<_>>();
    assert_eq!(covered.len(), RawFfiOp::ALL.len());

    let source = include_str!("../ffi.rs");
    let declared = source
        .split("bindings::")
        .skip(1)
        .filter_map(|tail| {
            let name = tail
                .split(|character: char| !(character.is_ascii_alphanumeric() || character == '_'))
                .next()?;
            let remainder = &tail[name.len()..];
            (name.starts_with("MV3D_LP_") && remainder.trim_start().starts_with('('))
                .then_some(name)
        })
        .collect::<HashSet<_>>();

    assert_eq!(
        covered, declared,
        "every production binding call needs a raw stub"
    );
}

// 验证每个 native FFI 失败保留操作符号与状态码，防止底层错误上下文丢失。
#[test]
fn every_native_ffi_failure_uses_the_expected_symbol_and_preserves_status() {
    let driver = NativeDriver;

    for &operation in RawFfiOp::ALL {
        match operation {
            RawFfiOp::GetVersion => exercise_version_failures(&driver),
            RawFfiOp::OpenDeviceByIp | RawFfiOp::OpenDeviceBySerial => {
                exercise_open_failure(&driver, operation, false);
                exercise_open_failure(&driver, operation, true);
            }
            _ => {
                configure(VersionResponse::Null, false);
                assert_eq!(
                    exercise_status_failure(&driver, operation),
                    Err(status_error())
                );
                assert_eq!(calls(), [operation]);
            }
        }
    }
}

// 验证非法图像输入在 native 符号前被拒绝，防止无效 descriptor 传入 SDK。
#[test]
fn invalid_image_input_is_rejected_before_calling_the_native_symbol() {
    configure(VersionResponse::Null, false);
    let padded_depth = [0_u8; 3];
    let input = ImageInput {
        image_type: ImageTypeRecord::from_raw(bindings::ImageType_Depth),
        width: 1,
        height: 1,
        data: &padded_depth,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };

    assert_eq!(
        NativeDriver.save_image(input, ImageFileFormatRecord::TiffU16, c"image.tiff"),
        Err(DriverError::InvalidInput(
            InvalidInput::InvalidImageLayout {
                field: "data length",
            }
        ))
    );
    assert!(calls().is_empty());
}

fn exercise_version_failures(driver: &NativeDriver) {
    configure(VersionResponse::Null, false);
    assert_eq!(
        driver.version(),
        Err(DriverError::Contract(ContractViolation::NullVersionPointer))
    );
    assert_eq!(calls(), [RawFfiOp::GetVersion]);

    configure(VersionResponse::Unterminated, false);
    assert_eq!(
        driver.version(),
        Err(DriverError::Contract(
            ContractViolation::UnterminatedVersion { limit: 64 }
        ))
    );
    assert_eq!(calls(), [RawFfiOp::GetVersion]);
}

fn exercise_open_failure(driver: &NativeDriver, operation: RawFfiOp, returns_handle: bool) {
    configure(VersionResponse::Null, returns_handle);
    let selector = c"192.0.2.1";
    let mut handle = None;
    let result = match operation {
        RawFfiOp::OpenDeviceByIp => driver.open_by_ip(selector, &mut handle),
        RawFfiOp::OpenDeviceBySerial => driver.open_by_serial(selector, &mut handle),
        _ => unreachable!("only open operations use this helper"),
    };
    assert_eq!(result, Err(status_error()));
    assert_eq!(handle.is_some(), returns_handle);
    assert_eq!(calls(), [operation]);
}

fn status_error() -> DriverError {
    DriverError::Status(INJECTED_STATUS)
}

fn exercise_status_failure(driver: &NativeDriver, operation: RawFfiOp) -> DriverResult<()> {
    let selector = c"value";
    let config = IpConfigRaw::from(&IpConfiguration::Dhcp);
    let handle = Handle::from_ptr(NonNull::<u8>::dangling().as_ptr().cast()).unwrap();
    let depth = [0_u8; 2];
    let image = ImageInput {
        image_type: ImageTypeRecord::from_raw(bindings::ImageType_Depth),
        width: 1,
        height: 1,
        data: &depth,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };

    match operation {
        RawFfiOp::GetVersion | RawFfiOp::OpenDeviceByIp | RawFfiOp::OpenDeviceBySerial => {
            unreachable!("special failure helper required")
        }
        RawFfiOp::Initialize => driver.initialize(),
        RawFfiOp::Finalize => driver.finalize(),
        RawFfiOp::GetDeviceNumber => driver.device_number().map(|_| ()),
        RawFfiOp::GetDeviceList => driver.device_list(1).map(|_| ()),
        RawFfiOp::SetIpConfig => driver.set_ip_config(selector, &config),
        RawFfiOp::CloseDevice => driver.close(handle),
        RawFfiOp::StartMeasure => driver.start(handle),
        RawFfiOp::StopMeasure => driver.stop(handle),
        RawFfiOp::SoftTrigger => driver.soft_trigger(handle),
        RawFfiOp::ClearDataBuffer => driver.clear_buffer(handle),
        RawFfiOp::GetImage => driver.get_image(handle, 1).map(|_| ()),
        RawFfiOp::RegisterImageCallback => {
            let sink: FrameCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
            let registration = CallbackRegistration::image(sink).unwrap();
            driver.register_image_callback(handle, registration.cookie())
        }
        RawFfiOp::RegisterExceptionCallback => {
            let sink: ExceptionCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
            let registration = CallbackRegistration::exception(sink).unwrap();
            driver.register_exception_callback(handle, registration.cookie())
        }
        RawFfiOp::GetParameter => driver.get_parameter(handle, selector).map(|_| ()),
        RawFfiOp::SetParameter => {
            driver.set_parameter(handle, selector, &ParameterValueRecord::Bool(true))
        }
        RawFfiOp::Execute => driver.execute(handle, selector),
        RawFfiOp::FileAccessRead => driver.file_access_read(handle, selector, selector),
        RawFfiOp::FileAccessWrite => driver.file_access_write(handle, selector, selector),
        RawFfiOp::GetFileAccessProgress => driver.file_access_progress(handle).map(|_| ()),
        RawFfiOp::MapDepthToPointCloud => driver.map_depth_to_point_cloud(image).map(|_| ()),
        RawFfiOp::MapDepthToPointCloudRound => driver
            .map_depth_to_point_cloud_round(std::slice::from_ref(&image))
            .map(|_| ()),
        RawFfiOp::ImageConvert => driver
            .convert_image(image, ImageTypeRecord::from_raw(bindings::ImageType_Mono8))
            .map(|_| ()),
        RawFfiOp::DepthMosaic => driver
            .mosaic_depth(std::slice::from_ref(&image))
            .map(|_| ()),
        RawFfiOp::SaveImage => driver.save_image(image, ImageFileFormatRecord::Bmp, c"image.bmp"),
        RawFfiOp::DisplayImage => {
            let mut raw = zeroed_image();
            // SAFETY: the test supplies initialized descriptor storage and the exact linked test
            // stub records the call without dereferencing the dummy window or image payload.
            unsafe {
                native_display_image_call(
                    &mut raw,
                    NonNull::<u8>::dangling().as_ptr().cast(),
                    bindings::DisplayType_Auto,
                    0,
                    0,
                )
            }
        }
    }
}

macro_rules! status_stub {
    ($rust_name:ident, $symbol:literal, $operation:expr $(, $argument:ident: $argument_type:ty)*) => {
        #[unsafe(export_name = $symbol)]
        pub extern "C" fn $rust_name($($argument: $argument_type),*) -> bindings::MV3D_LP_STATUS {
            $(let _ = $argument;)*
            record($operation)
        }
    };
}

#[unsafe(export_name = "MV3D_LP_GetVersion")]
pub extern "C" fn stub_get_version() -> *const c_char {
    record(RawFfiOp::GetVersion);
    match version_response() {
        VersionResponse::Null => ptr::null(),
        VersionResponse::Unterminated => UNTERMINATED_VERSION.as_ptr().cast(),
    }
}

status_stub!(stub_initialize, "MV3D_LP_Initialize", RawFfiOp::Initialize);
status_stub!(stub_finalize, "MV3D_LP_Finalize", RawFfiOp::Finalize);

#[unsafe(export_name = "MV3D_LP_GetDeviceNumber")]
pub extern "C" fn stub_get_device_number(output: *mut u32) -> bindings::MV3D_LP_STATUS {
    write_output(output, u32::MAX);
    record(RawFfiOp::GetDeviceNumber)
}

#[unsafe(export_name = "MV3D_LP_GetDeviceList")]
pub extern "C" fn stub_get_device_list(
    _devices: *mut bindings::MV3D_LP_DEVICE_INFO,
    _capacity: u32,
    reported: *mut u32,
) -> bindings::MV3D_LP_STATUS {
    write_output(reported, u32::MAX);
    record(RawFfiOp::GetDeviceList)
}

status_stub!(
    stub_set_ip_config,
    "MV3D_LP_SetIpConfig",
    RawFfiOp::SetIpConfig,
    serial: *const c_char,
    config: *mut bindings::MV3D_LP_IP_CONFIG
);

fn stub_open(output: *mut bindings::HANDLE, operation: RawFfiOp) -> bindings::MV3D_LP_STATUS {
    let value = if open_returns_handle() {
        NonNull::<u8>::dangling().as_ptr().cast()
    } else {
        ptr::null_mut()
    };
    write_output(output, value);
    record(operation)
}

#[unsafe(export_name = "MV3D_LP_OpenDeviceByIP")]
pub extern "C" fn stub_open_by_ip(
    output: *mut bindings::HANDLE,
    _selector: *const c_char,
) -> bindings::MV3D_LP_STATUS {
    stub_open(output, RawFfiOp::OpenDeviceByIp)
}

#[unsafe(export_name = "MV3D_LP_OpenDeviceBySN")]
pub extern "C" fn stub_open_by_serial(
    output: *mut bindings::HANDLE,
    _selector: *const c_char,
) -> bindings::MV3D_LP_STATUS {
    stub_open(output, RawFfiOp::OpenDeviceBySerial)
}

status_stub!(
    stub_close_device,
    "MV3D_LP_CloseDevice",
    RawFfiOp::CloseDevice,
    handle: *mut bindings::HANDLE
);
status_stub!(
    stub_start_measure,
    "MV3D_LP_StartMeasure",
    RawFfiOp::StartMeasure,
    handle: bindings::HANDLE
);
status_stub!(
    stub_stop_measure,
    "MV3D_LP_StopMeasure",
    RawFfiOp::StopMeasure,
    handle: bindings::HANDLE
);
status_stub!(
    stub_soft_trigger,
    "MV3D_LP_SoftTrigger",
    RawFfiOp::SoftTrigger,
    handle: bindings::HANDLE
);
status_stub!(
    stub_clear_data_buffer,
    "MV3D_LP_ClearDataBuffer",
    RawFfiOp::ClearDataBuffer,
    handle: bindings::HANDLE
);

#[unsafe(export_name = "MV3D_LP_GetImage")]
pub extern "C" fn stub_get_image(
    _handle: bindings::HANDLE,
    output: *mut bindings::MV3D_LP_IMAGE_DATA,
    _timeout: u32,
) -> bindings::MV3D_LP_STATUS {
    write_output(output, poisoned_image());
    record(RawFfiOp::GetImage)
}

status_stub!(
    stub_register_image_callback,
    "MV3D_LP_RegisterImageDataCallBack",
    RawFfiOp::RegisterImageCallback,
    handle: bindings::HANDLE,
    callback: bindings::MV3D_LP_ImageDataCallBack,
    user: *mut c_void
);
status_stub!(
    stub_register_exception_callback,
    "MV3D_LP_RegisterExceptionCallBack",
    RawFfiOp::RegisterExceptionCallback,
    handle: bindings::HANDLE,
    callback: bindings::MV3D_LP_ExceptionCallBack,
    user: *mut c_void
);

#[unsafe(export_name = "MV3D_LP_GetParam")]
pub extern "C" fn stub_get_parameter(
    _handle: bindings::HANDLE,
    _key: *const c_char,
    output: *mut bindings::MV3D_LP_PARAM,
) -> bindings::MV3D_LP_STATUS {
    let mut parameter = zeroed_parameter();
    parameter.enParamType = i32::MAX;
    write_output(output, parameter);
    record(RawFfiOp::GetParameter)
}

status_stub!(
    stub_set_parameter,
    "MV3D_LP_SetParam",
    RawFfiOp::SetParameter,
    handle: bindings::HANDLE,
    key: *const c_char,
    parameter: *mut bindings::MV3D_LP_PARAM
);
status_stub!(
    stub_execute,
    "MV3D_LP_Execute",
    RawFfiOp::Execute,
    handle: bindings::HANDLE,
    key: *const c_char
);
status_stub!(
    stub_file_access_read,
    "MV3D_LP_FileAccessRead",
    RawFfiOp::FileAccessRead,
    handle: bindings::HANDLE,
    access: *mut bindings::MV3D_LP_FILE_ACCESS
);
status_stub!(
    stub_file_access_write,
    "MV3D_LP_FileAccessWrite",
    RawFfiOp::FileAccessWrite,
    handle: bindings::HANDLE,
    access: *mut bindings::MV3D_LP_FILE_ACCESS
);

#[unsafe(export_name = "MV3D_LP_GetFileAccessProgress")]
pub extern "C" fn stub_file_access_progress(
    _handle: bindings::HANDLE,
    output: *mut bindings::MV3D_LP_FILE_ACCESS_PROGRESS,
) -> bindings::MV3D_LP_STATUS {
    write_output(
        output,
        bindings::MV3D_LP_FILE_ACCESS_PROGRESS {
            nCompleted: i64::MAX,
            nTotal: -1,
            nReserved: [0xFF; 32],
        },
    );
    record(RawFfiOp::GetFileAccessProgress)
}

#[unsafe(export_name = "MV3D_LP_MapDepthToPointCloud")]
pub extern "C" fn stub_map_depth_to_point_cloud(
    _input: *mut bindings::MV3D_LP_IMAGE_DATA,
    output: *mut bindings::MV3D_LP_IMAGE_DATA,
) -> bindings::MV3D_LP_STATUS {
    write_output(output, poisoned_image());
    record(RawFfiOp::MapDepthToPointCloud)
}

#[unsafe(export_name = "MV3D_LP_MapDepthToPointCloudRound")]
pub extern "C" fn stub_map_depth_to_point_cloud_round(
    _inputs: *mut bindings::MV3D_LP_IMAGE_DATA,
    _count: u32,
    output: *mut bindings::MV3D_LP_IMAGE_DATA,
) -> bindings::MV3D_LP_STATUS {
    write_output(output, poisoned_image());
    record(RawFfiOp::MapDepthToPointCloudRound)
}

#[unsafe(export_name = "MV3D_LP_ImageConvert")]
pub extern "C" fn stub_image_convert(
    _input: *mut bindings::MV3D_LP_IMAGE_DATA,
    output: *mut bindings::MV3D_LP_IMAGE_DATA,
) -> bindings::MV3D_LP_STATUS {
    write_output(output, poisoned_image());
    record(RawFfiOp::ImageConvert)
}

#[unsafe(export_name = "MV3D_LP_DepthMosaic")]
pub extern "C" fn stub_depth_mosaic(
    _inputs: *mut bindings::MV3D_LP_IMAGE_DATA,
    _count: u32,
    output: *mut bindings::MV3D_LP_IMAGE_DATA,
) -> bindings::MV3D_LP_STATUS {
    write_output(output, poisoned_image());
    record(RawFfiOp::DepthMosaic)
}

status_stub!(
    stub_save_image,
    "MV3D_LP_SaveImage",
    RawFfiOp::SaveImage,
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    format: bindings::Mv3dLpFileType,
    file_name: *const c_char
);
status_stub!(
    stub_display_image,
    "MV3D_LP_DisplayImage",
    RawFfiOp::DisplayImage,
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    window: *mut c_void,
    display_type: bindings::Mv3dLpDisplayType,
    minimum: i32,
    maximum: i32
);
