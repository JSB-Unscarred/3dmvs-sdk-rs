use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::ffi::CStr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::callback::CallbackCookie;
use crate::device::{DeviceListAttempt, IpConfigRaw};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{Driver, DriverResult, Handle};
use crate::file_transfer::FileProgressRaw;
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::{Gate, Runtime};

#[derive(Clone)]
pub(crate) struct MockDriver {
    shared: Rc<RefCell<MockState>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FileAccessCall {
    pub operation: &'static str,
    pub user_file_name: Vec<u8>,
    pub device_file_name: Vec<u8>,
    pub user_file_name_address: usize,
    pub device_file_name_address: usize,
}

#[cfg(feature = "display-windows")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DisplayCall {
    pub window: NonZeroIsize,
    pub range: DisplayRangeRecord,
}

struct MockState {
    log: Vec<&'static str>,
    capacities: Vec<usize>,
    version: DriverResult<Vec<u8>>,
    initialize: VecDeque<DriverResult<()>>,
    finalize: VecDeque<DriverResult<()>>,
    device_number: VecDeque<DriverResult<u32>>,
    device_list: VecDeque<DriverResult<DeviceListAttempt>>,
    open_handle: Option<Handle>,
    open: VecDeque<DriverResult<()>>,
    close: VecDeque<DriverResult<()>>,
    close_entered: Option<Arc<AtomicBool>>,
    start: VecDeque<DriverResult<()>>,
    stop: VecDeque<DriverResult<()>>,
    stop_entered: Option<Arc<AtomicBool>>,
    soft_trigger: VecDeque<DriverResult<()>>,
    clear_buffer: VecDeque<DriverResult<()>>,
    get_image: VecDeque<DriverResult<FrameRecord>>,
    image_timeouts: Vec<u32>,
    register_image_callback: VecDeque<DriverResult<()>>,
    register_exception_callback: VecDeque<DriverResult<()>>,
    image_callback_cookies: Vec<CallbackCookie>,
    exception_callback_cookies: Vec<CallbackCookie>,
    get_parameter: VecDeque<DriverResult<ParameterRecord>>,
    set_parameter: VecDeque<DriverResult<()>>,
    execute: VecDeque<DriverResult<()>>,
    file_access_read: VecDeque<DriverResult<()>>,
    file_access_write: VecDeque<DriverResult<()>>,
    file_access_progress: VecDeque<DriverResult<FileProgressRaw>>,
    file_access_calls: Vec<FileAccessCall>,
    map_depth_to_point_cloud: VecDeque<DriverResult<FrameRecord>>,
    map_depth_to_point_cloud_round: VecDeque<DriverResult<FrameRecord>>,
    convert_image: VecDeque<DriverResult<FrameRecord>>,
    mosaic_depth: VecDeque<DriverResult<FrameRecord>>,
    save_image: VecDeque<DriverResult<()>>,
    #[cfg(feature = "display-windows")]
    display_image: VecDeque<DriverResult<()>>,
    #[cfg(feature = "display-windows")]
    display_calls: Vec<DisplayCall>,
}

impl MockDriver {
    pub(crate) fn new() -> Self {
        Self {
            shared: Rc::new(RefCell::new(MockState {
                log: Vec::new(),
                capacities: Vec::new(),
                version: Ok(b"1.3.3.3".to_vec()),
                initialize: VecDeque::new(),
                finalize: VecDeque::new(),
                device_number: VecDeque::new(),
                device_list: VecDeque::new(),
                open_handle: Some(mock_handle(1)),
                open: VecDeque::new(),
                close: VecDeque::new(),
                close_entered: None,
                start: VecDeque::new(),
                stop: VecDeque::new(),
                stop_entered: None,
                soft_trigger: VecDeque::new(),
                clear_buffer: VecDeque::new(),
                get_image: VecDeque::new(),
                image_timeouts: Vec::new(),
                register_image_callback: VecDeque::new(),
                register_exception_callback: VecDeque::new(),
                image_callback_cookies: Vec::new(),
                exception_callback_cookies: Vec::new(),
                get_parameter: VecDeque::new(),
                set_parameter: VecDeque::new(),
                execute: VecDeque::new(),
                file_access_read: VecDeque::new(),
                file_access_write: VecDeque::new(),
                file_access_progress: VecDeque::new(),
                file_access_calls: Vec::new(),
                map_depth_to_point_cloud: VecDeque::new(),
                map_depth_to_point_cloud_round: VecDeque::new(),
                convert_image: VecDeque::new(),
                mosaic_depth: VecDeque::new(),
                save_image: VecDeque::new(),
                #[cfg(feature = "display-windows")]
                display_image: VecDeque::new(),
                #[cfg(feature = "display-windows")]
                display_calls: Vec::new(),
            })),
        }
    }

    pub(crate) fn set_version(&self, result: DriverResult<Vec<u8>>) {
        self.state().version = result;
    }

    pub(crate) fn push_initialize(&self, result: DriverResult<()>) {
        self.state().initialize.push_back(result);
    }

    pub(crate) fn push_finalize(&self, result: DriverResult<()>) {
        self.state().finalize.push_back(result);
    }

    pub(crate) fn push_device_number(&self, result: DriverResult<u32>) {
        self.state().device_number.push_back(result);
    }

    pub(crate) fn push_device_list(&self, result: DriverResult<DeviceListAttempt>) {
        self.state().device_list.push_back(result);
    }

    pub(crate) fn configure_open(&self, handle: Option<Handle>, result: DriverResult<()>) {
        let mut state = self.state();
        state.open_handle = handle;
        state.open.push_back(result);
    }

    pub(crate) fn push_close(&self, result: DriverResult<()>) {
        self.state().close.push_back(result);
    }

    pub(crate) fn set_close_entered(&self, entered: Arc<AtomicBool>) {
        self.state().close_entered = Some(entered);
    }

    pub(crate) fn push_start(&self, result: DriverResult<()>) {
        self.state().start.push_back(result);
    }

    pub(crate) fn push_stop(&self, result: DriverResult<()>) {
        self.state().stop.push_back(result);
    }

    pub(crate) fn set_stop_entered(&self, entered: Arc<AtomicBool>) {
        self.state().stop_entered = Some(entered);
    }

    pub(crate) fn push_soft_trigger(&self, result: DriverResult<()>) {
        self.state().soft_trigger.push_back(result);
    }

    pub(crate) fn push_clear_buffer(&self, result: DriverResult<()>) {
        self.state().clear_buffer.push_back(result);
    }

    pub(crate) fn push_get_image(&self, result: DriverResult<FrameRecord>) {
        self.state().get_image.push_back(result);
    }

    pub(crate) fn push_register_image_callback(&self, result: DriverResult<()>) {
        self.state().register_image_callback.push_back(result);
    }

    pub(crate) fn push_register_exception_callback(&self, result: DriverResult<()>) {
        self.state().register_exception_callback.push_back(result);
    }

    pub(crate) fn push_get_parameter(&self, result: DriverResult<ParameterRecord>) {
        self.state().get_parameter.push_back(result);
    }

    pub(crate) fn push_file_access_read(&self, result: DriverResult<()>) {
        self.state().file_access_read.push_back(result);
    }

    pub(crate) fn push_file_access_write(&self, result: DriverResult<()>) {
        self.state().file_access_write.push_back(result);
    }

    pub(crate) fn push_file_access_progress(&self, result: DriverResult<FileProgressRaw>) {
        self.state().file_access_progress.push_back(result);
    }

    pub(crate) fn push_map_depth_to_point_cloud(&self, result: DriverResult<FrameRecord>) {
        self.state().map_depth_to_point_cloud.push_back(result);
    }

    pub(crate) fn push_map_depth_to_point_cloud_round(&self, result: DriverResult<FrameRecord>) {
        self.state()
            .map_depth_to_point_cloud_round
            .push_back(result);
    }

    pub(crate) fn push_convert_image(&self, result: DriverResult<FrameRecord>) {
        self.state().convert_image.push_back(result);
    }

    pub(crate) fn push_mosaic_depth(&self, result: DriverResult<FrameRecord>) {
        self.state().mosaic_depth.push_back(result);
    }

    pub(crate) fn push_save_image(&self, result: DriverResult<()>) {
        self.state().save_image.push_back(result);
    }

    #[cfg(feature = "display-windows")]
    pub(crate) fn push_display_image(&self, result: DriverResult<()>) {
        self.state().display_image.push_back(result);
    }

    pub(crate) fn logs(&self) -> Vec<&'static str> {
        self.state().log.clone()
    }

    pub(crate) fn capacities(&self) -> Vec<usize> {
        self.state().capacities.clone()
    }

    pub(crate) fn image_timeouts(&self) -> Vec<u32> {
        self.state().image_timeouts.clone()
    }

    pub(crate) fn image_callback_cookies(&self) -> Vec<CallbackCookie> {
        self.state().image_callback_cookies.clone()
    }

    pub(crate) fn exception_callback_cookies(&self) -> Vec<CallbackCookie> {
        self.state().exception_callback_cookies.clone()
    }

    pub(crate) fn file_access_calls(&self) -> Vec<FileAccessCall> {
        self.state().file_access_calls.clone()
    }

    #[cfg(feature = "display-windows")]
    pub(crate) fn display_calls(&self) -> Vec<DisplayCall> {
        self.state().display_calls.clone()
    }

    fn state(&self) -> RefMut<'_, MockState> {
        self.shared.borrow_mut()
    }
}

pub(crate) fn active_runtime(mock: &MockDriver) -> (Runtime, Arc<Gate>) {
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate))
        .expect("default mock initialization should succeed");
    (runtime, gate)
}

impl Driver for MockDriver {
    fn version(&self) -> DriverResult<Vec<u8>> {
        let mut state = self.state();
        state.log.push("version");
        state.version.clone()
    }

    fn initialize(&self) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("initialize");
        pop_or(&mut state.initialize, Ok(()))
    }

    fn finalize(&self) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("finalize");
        pop_or(&mut state.finalize, Ok(()))
    }

    fn device_number(&self) -> DriverResult<u32> {
        let mut state = self.state();
        state.log.push("device_number");
        pop_or(&mut state.device_number, Ok(0))
    }

    fn device_list(&self, capacity: usize) -> DriverResult<DeviceListAttempt> {
        let mut state = self.state();
        state.log.push("device_list");
        state.capacities.push(capacity);
        pop_or(
            &mut state.device_list,
            Ok(DeviceListAttempt {
                records: Vec::new(),
                reported: 0,
            }),
        )
    }

    fn set_ip_config(&self, _: &CStr, _: &IpConfigRaw) -> DriverResult<()> {
        self.state().log.push("set_ip_config");
        Ok(())
    }

    fn open_by_ip(&self, _: &CStr, handle: &mut Option<Handle>) -> DriverResult<()> {
        self.open("open_by_ip", handle)
    }

    fn open_by_serial(&self, _: &CStr, handle: &mut Option<Handle>) -> DriverResult<()> {
        self.open("open_by_serial", handle)
    }

    fn close(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("close");
        if let Some(entered) = &state.close_entered {
            entered.store(true, Ordering::SeqCst);
        }
        pop_or(&mut state.close, Ok(()))
    }

    fn start(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("start");
        pop_or(&mut state.start, Ok(()))
    }

    fn stop(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("stop");
        if let Some(entered) = &state.stop_entered {
            entered.store(true, Ordering::SeqCst);
        }
        pop_or(&mut state.stop, Ok(()))
    }

    fn soft_trigger(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("soft_trigger");
        pop_or(&mut state.soft_trigger, Ok(()))
    }

    fn clear_buffer(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("clear_buffer");
        pop_or(&mut state.clear_buffer, Ok(()))
    }

    fn get_image(&self, _: Handle, timeout_ms: u32) -> DriverResult<FrameRecord> {
        let mut state = self.state();
        state.log.push("get_image");
        state.image_timeouts.push(timeout_ms);
        pop_or(
            &mut state.get_image,
            Err(crate::driver::DriverError::Status(0x8006_0006_u32 as i32)),
        )
    }

    fn register_image_callback(&self, _: Handle, cookie: CallbackCookie) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("register_image_callback");
        state.image_callback_cookies.push(cookie);
        pop_or(&mut state.register_image_callback, Ok(()))
    }

    fn register_exception_callback(&self, _: Handle, cookie: CallbackCookie) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("register_exception_callback");
        state.exception_callback_cookies.push(cookie);
        pop_or(&mut state.register_exception_callback, Ok(()))
    }

    fn get_parameter(&self, _: Handle, _: &CStr) -> DriverResult<ParameterRecord> {
        let mut state = self.state();
        state.log.push("get_parameter");
        pop_or(&mut state.get_parameter, Ok(ParameterRecord::Bool(false)))
    }

    fn set_parameter(&self, _: Handle, _: &CStr, _: &ParameterValueRecord) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("set_parameter");
        pop_or(&mut state.set_parameter, Ok(()))
    }

    fn execute(&self, _: Handle, _: &CStr) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("execute");
        pop_or(&mut state.execute, Ok(()))
    }

    fn file_access_read(
        &self,
        _: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("file_access_read");
        state.file_access_calls.push(FileAccessCall {
            operation: "file_access_read",
            user_file_name: user_file_name.to_bytes().to_vec(),
            device_file_name: device_file_name.to_bytes().to_vec(),
            user_file_name_address: user_file_name.as_ptr() as usize,
            device_file_name_address: device_file_name.as_ptr() as usize,
        });
        pop_or(&mut state.file_access_read, Ok(()))
    }

    fn file_access_write(
        &self,
        _: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("file_access_write");
        state.file_access_calls.push(FileAccessCall {
            operation: "file_access_write",
            user_file_name: user_file_name.to_bytes().to_vec(),
            device_file_name: device_file_name.to_bytes().to_vec(),
            user_file_name_address: user_file_name.as_ptr() as usize,
            device_file_name_address: device_file_name.as_ptr() as usize,
        });
        pop_or(&mut state.file_access_write, Ok(()))
    }

    fn file_access_progress(&self, _: Handle) -> DriverResult<FileProgressRaw> {
        let mut state = self.state();
        state.log.push("file_access_progress");
        pop_or(
            &mut state.file_access_progress,
            Ok(FileProgressRaw {
                completed: 0,
                total: 0,
            }),
        )
    }

    fn map_depth_to_point_cloud(&self, _: ImageInput<'_>) -> DriverResult<FrameRecord> {
        let mut state = self.state();
        state.log.push("map_depth_to_point_cloud");
        pop_or(
            &mut state.map_depth_to_point_cloud,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn map_depth_to_point_cloud_round(&self, _: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        let mut state = self.state();
        state.log.push("map_depth_to_point_cloud_round");
        pop_or(
            &mut state.map_depth_to_point_cloud_round,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn convert_image(&self, _: ImageInput<'_>, _: ImageTypeRecord) -> DriverResult<FrameRecord> {
        let mut state = self.state();
        state.log.push("convert_image");
        pop_or(
            &mut state.convert_image,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn mosaic_depth(&self, _: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        let mut state = self.state();
        state.log.push("mosaic_depth");
        pop_or(
            &mut state.mosaic_depth,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn save_image(
        &self,
        _: ImageInput<'_>,
        _: ImageFileFormatRecord,
        _: &CStr,
    ) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("save_image");
        pop_or(&mut state.save_image, Ok(()))
    }

    #[cfg(feature = "display-windows")]
    fn display_image(
        &self,
        _: ImageInput<'_>,
        window: NonZeroIsize,
        range: DisplayRangeRecord,
    ) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push("display_image");
        state.display_calls.push(DisplayCall { window, range });
        pop_or(&mut state.display_image, Ok(()))
    }
}

impl MockDriver {
    fn open(&self, operation: &'static str, output: &mut Option<Handle>) -> DriverResult<()> {
        let mut state = self.state();
        state.log.push(operation);
        *output = state.open_handle;
        pop_or(&mut state.open, Ok(()))
    }
}

fn pop_or<T>(queue: &mut VecDeque<DriverResult<T>>, default: DriverResult<T>) -> DriverResult<T> {
    queue.pop_front().unwrap_or(default)
}

pub(crate) fn mock_handle(address: usize) -> Handle {
    Handle::from_ptr(address as *mut std::ffi::c_void)
        .expect("a mock handle address must be non-zero")
}
