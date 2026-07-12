use std::cell::{RefCell, RefMut};
use std::collections::VecDeque;
use std::ffi::CStr;
use std::rc::Rc;
use std::sync::Arc;

use crate::device::{DeviceListAttempt, IpConfigRaw};
use crate::driver::{Driver, DriverResult, Handle};
use crate::frame::FrameRecord;
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::{Gate, Runtime};

#[derive(Clone)]
pub(crate) struct MockDriver {
    shared: Rc<RefCell<MockState>>,
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
    start: VecDeque<DriverResult<()>>,
    stop: VecDeque<DriverResult<()>>,
    soft_trigger: VecDeque<DriverResult<()>>,
    clear_buffer: VecDeque<DriverResult<()>>,
    get_image: VecDeque<DriverResult<FrameRecord>>,
    image_timeouts: Vec<u32>,
    get_parameter: VecDeque<DriverResult<ParameterRecord>>,
    set_parameter: VecDeque<DriverResult<()>>,
    execute: VecDeque<DriverResult<()>>,
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
                start: VecDeque::new(),
                stop: VecDeque::new(),
                soft_trigger: VecDeque::new(),
                clear_buffer: VecDeque::new(),
                get_image: VecDeque::new(),
                image_timeouts: Vec::new(),
                get_parameter: VecDeque::new(),
                set_parameter: VecDeque::new(),
                execute: VecDeque::new(),
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

    pub(crate) fn push_start(&self, result: DriverResult<()>) {
        self.state().start.push_back(result);
    }

    pub(crate) fn push_stop(&self, result: DriverResult<()>) {
        self.state().stop.push_back(result);
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

    pub(crate) fn push_get_parameter(&self, result: DriverResult<ParameterRecord>) {
        self.state().get_parameter.push_back(result);
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
