#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ffi::CStr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::ptr::NonNull;

use crate::callback::CallbackCookie;
use crate::device::{DeviceInfoRaw, DeviceListAttempt, IpConfigRaw};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::error::{ContractViolation, InvalidInput};
use crate::file_transfer::FileProgressRaw;
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::parameter::{ParameterRecord, ParameterValueRecord};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DriverError {
    Status(i32),
    InvalidInput(InvalidInput),
    Contract(ContractViolation),
    Allocation { requested: usize },
}

pub(crate) type DriverResult<T> = Result<T, DriverError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Handle(NonNull<std::ffi::c_void>);

// SAFETY: Production handles are opaque values returned by successful device opens.
// Rust never dereferences them, and the crate's safe public API does not expose the raw
// value. `Handle` deliberately has no `Sync` implementation, so every device/session owner
// inherits `!Sync` and calls for one handle cannot overlap through the safe API. Vendor evidence
// for moving handles between threads and operating distinct devices concurrently is summarized in
// `README.md` under "原生契约假设"; the release acceptance scope is recorded in
// `docs/threading/lpsdk-1.3.3.3-native-acceptance.md`.
unsafe impl Send for Handle {}

impl Handle {
    pub(crate) fn from_ptr(pointer: *mut std::ffi::c_void) -> Option<Self> {
        NonNull::new(pointer).map(Self)
    }

    pub(crate) fn as_ptr(self) -> *mut std::ffi::c_void {
        self.0.as_ptr()
    }
}

/// Native operation surface shared by the process owner and owned device leases.
///
/// `Send` lets an `Arc<RuntimeCore>` travel with a uniquely owned device; `Sync` permits distinct
/// device handles to call the process-wide driver concurrently.
pub(crate) trait Driver: Send + Sync {
    fn version(&self) -> DriverResult<Vec<u8>>;
    fn initialize(&self) -> DriverResult<()>;
    fn finalize(&self) -> DriverResult<()>;

    fn device_number(&self) -> DriverResult<u32>;
    fn device_list(&self, capacity: usize) -> DriverResult<DeviceListAttempt>;
    fn set_ip_config(&self, serial: &CStr, config: &IpConfigRaw) -> DriverResult<()>;

    fn open_by_ip(&self, ip: &CStr) -> DriverResult<Handle>;
    fn open_by_serial(&self, serial: &CStr) -> DriverResult<Handle>;
    fn close(&self, handle: Handle) -> DriverResult<()>;

    fn start(&self, handle: Handle) -> DriverResult<()>;
    fn stop(&self, handle: Handle) -> DriverResult<()>;
    fn soft_trigger(&self, handle: Handle) -> DriverResult<()>;
    fn clear_buffer(&self, handle: Handle) -> DriverResult<()>;
    fn get_image(&self, handle: Handle, timeout_ms: u32) -> DriverResult<FrameRecord>;
    fn register_image_callback(&self, handle: Handle, cookie: CallbackCookie) -> DriverResult<()>;
    fn register_exception_callback(
        &self,
        handle: Handle,
        cookie: CallbackCookie,
    ) -> DriverResult<()>;

    fn get_parameter(&self, handle: Handle, key: &CStr) -> DriverResult<ParameterRecord>;
    fn set_parameter(
        &self,
        handle: Handle,
        key: &CStr,
        value: &ParameterValueRecord,
    ) -> DriverResult<()>;
    fn execute(&self, handle: Handle, key: &CStr) -> DriverResult<()>;
    fn file_access_read(
        &self,
        handle: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()>;
    fn file_access_write(
        &self,
        handle: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()>;
    fn file_access_progress(&self, handle: Handle) -> DriverResult<FileProgressRaw>;

    fn map_depth_to_point_cloud(&self, input: ImageInput<'_>) -> DriverResult<FrameRecord>;
    fn map_depth_to_point_cloud_round(
        &self,
        inputs: &[ImageInput<'_>],
    ) -> DriverResult<FrameRecord>;
    fn convert_image(
        &self,
        input: ImageInput<'_>,
        target: ImageTypeRecord,
    ) -> DriverResult<FrameRecord>;
    fn mosaic_depth(&self, inputs: &[ImageInput<'_>]) -> DriverResult<FrameRecord>;
    fn save_image(
        &self,
        input: ImageInput<'_>,
        format: ImageFileFormatRecord,
        file_name: &CStr,
    ) -> DriverResult<()>;
    #[cfg(feature = "display-windows")]
    fn display_image(
        &self,
        input: ImageInput<'_>,
        window: NonZeroIsize,
        range: DisplayRangeRecord,
    ) -> DriverResult<()>;
}

pub(crate) fn status_result(status: i32) -> DriverResult<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(DriverError::Status(status))
    }
}

#[allow(dead_code)]
fn _raw_device_type_is_owned(_: DeviceInfoRaw) {}
