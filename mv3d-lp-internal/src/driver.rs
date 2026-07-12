#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ffi::CStr;
use std::ptr::NonNull;

use crate::device::{DeviceInfoRaw, DeviceListAttempt, IpConfigRaw};
use crate::error::ContractViolation;
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DriverError {
    Status(i32),
    Contract(ContractViolation),
    Allocation { requested: usize },
    OrphanedHandle(Box<DriverError>),
}

pub(crate) type DriverResult<T> = Result<T, DriverError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Handle(NonNull<std::ffi::c_void>);

impl Handle {
    pub(crate) fn from_ptr(pointer: *mut std::ffi::c_void) -> Option<Self> {
        NonNull::new(pointer).map(Self)
    }

    pub(crate) fn as_ptr(self) -> *mut std::ffi::c_void {
        self.0.as_ptr()
    }
}

pub(crate) trait Driver {
    fn version(&self) -> DriverResult<Vec<u8>>;
    fn initialize(&self) -> DriverResult<()>;
    fn finalize(&self) -> DriverResult<()>;

    fn device_number(&self) -> DriverResult<u32>;
    fn device_list(&self, capacity: usize) -> DriverResult<DeviceListAttempt>;
    fn set_ip_config(&self, serial: &CStr, config: &IpConfigRaw) -> DriverResult<()>;

    fn open_by_ip(&self, ip: &CStr, handle: &mut Option<Handle>) -> DriverResult<()>;
    fn open_by_serial(&self, serial: &CStr, handle: &mut Option<Handle>) -> DriverResult<()>;
    fn close(&self, handle: Handle) -> DriverResult<()>;

    fn start(&self, handle: Handle) -> DriverResult<()>;
    fn stop(&self, handle: Handle) -> DriverResult<()>;
    fn soft_trigger(&self, handle: Handle) -> DriverResult<()>;
    fn clear_buffer(&self, handle: Handle) -> DriverResult<()>;
    fn get_image(&self, handle: Handle, timeout_ms: u32) -> DriverResult<FrameRecord>;

    fn get_parameter(&self, handle: Handle, key: &CStr) -> DriverResult<ParameterRecord>;
    fn set_parameter(
        &self,
        handle: Handle,
        key: &CStr,
        value: &ParameterValueRecord,
    ) -> DriverResult<()>;
    fn execute(&self, handle: Handle, key: &CStr) -> DriverResult<()>;
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
