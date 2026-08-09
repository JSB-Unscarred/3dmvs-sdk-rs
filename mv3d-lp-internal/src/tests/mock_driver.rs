use std::collections::VecDeque;
use std::ffi::CStr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::{Arc, Mutex, MutexGuard};

use crate::callback::CallbackCookie;
use crate::device::{DeviceListAttempt, IpConfigRaw};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{Driver, DriverError, DriverResult, Handle};
use crate::file_transfer::FileProgressRaw;
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::{Gate, Runtime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Call {
    Version,
    Initialize,
    Finalize,
    DeviceNumber,
    DeviceList,
    OpenByIp,
    OpenBySerial,
    Close,
    Start,
    Stop,
    SoftTrigger,
    ClearBuffer,
    GetImage,
    RegisterImageCallback,
    RegisterExceptionCallback,
    FileRead,
    FileWrite,
    FileProgress,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FileCall {
    pub direction: Call,
    pub user_file_name: Vec<u8>,
    pub device_file_name: Vec<u8>,
}

#[derive(Clone)]
pub(crate) struct MockDriver {
    state: Arc<Mutex<State>>,
}

struct State {
    calls: Vec<Call>,
    capacities: Vec<usize>,
    version: DriverResult<Vec<u8>>,
    initialize: VecDeque<DriverResult<()>>,
    finalize: VecDeque<DriverResult<()>>,
    device_number: VecDeque<DriverResult<u32>>,
    device_list: VecDeque<DriverResult<DeviceListAttempt>>,
    close: VecDeque<DriverResult<()>>,
    stop: VecDeque<DriverResult<()>>,
    get_image: VecDeque<DriverResult<FrameRecord>>,
    image_timeouts: Vec<u32>,
    file_progress: VecDeque<DriverResult<FileProgressRaw>>,
    file_calls: Vec<FileCall>,
    next_handle: usize,
}

impl MockDriver {
    pub(crate) fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(State {
                calls: Vec::new(),
                capacities: Vec::new(),
                version: Ok(b"1.3.3.3".to_vec()),
                initialize: VecDeque::new(),
                finalize: VecDeque::new(),
                device_number: VecDeque::new(),
                device_list: VecDeque::new(),
                close: VecDeque::new(),
                stop: VecDeque::new(),
                get_image: VecDeque::new(),
                image_timeouts: Vec::new(),
                file_progress: VecDeque::new(),
                file_calls: Vec::new(),
                next_handle: 1,
            })),
        }
    }

    pub(crate) fn set_version(&self, result: DriverResult<Vec<u8>>) {
        self.lock().version = result;
    }

    pub(crate) fn push_initialize(&self, result: DriverResult<()>) {
        self.lock().initialize.push_back(result);
    }

    pub(crate) fn push_finalize(&self, result: DriverResult<()>) {
        self.lock().finalize.push_back(result);
    }

    pub(crate) fn push_device_number(&self, result: DriverResult<u32>) {
        self.lock().device_number.push_back(result);
    }

    pub(crate) fn push_device_list(&self, result: DriverResult<DeviceListAttempt>) {
        self.lock().device_list.push_back(result);
    }

    pub(crate) fn push_stop(&self, result: DriverResult<()>) {
        self.lock().stop.push_back(result);
    }

    pub(crate) fn push_close(&self, result: DriverResult<()>) {
        self.lock().close.push_back(result);
    }

    pub(crate) fn push_get_image(&self, result: DriverResult<FrameRecord>) {
        self.lock().get_image.push_back(result);
    }

    pub(crate) fn push_file_progress(&self, result: DriverResult<FileProgressRaw>) {
        self.lock().file_progress.push_back(result);
    }

    pub(crate) fn calls(&self) -> Vec<Call> {
        self.lock().calls.clone()
    }

    pub(crate) fn capacities(&self) -> Vec<usize> {
        self.lock().capacities.clone()
    }

    pub(crate) fn image_timeouts(&self) -> Vec<u32> {
        self.lock().image_timeouts.clone()
    }

    pub(crate) fn file_calls(&self) -> Vec<FileCall> {
        self.lock().file_calls.clone()
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn record(&self, call: Call) {
        self.lock().calls.push(call);
    }

    fn open(&self, call: Call) -> DriverResult<Handle> {
        let mut state = self.lock();
        state.calls.push(call);
        let handle = mock_handle(state.next_handle);
        state.next_handle += 1;
        Ok(handle)
    }
}

pub(crate) fn active_runtime(mock: &MockDriver) -> (Runtime, Arc<Gate>) {
    let gate = Arc::new(Gate::new());
    let runtime = Runtime::initialize_with(Box::new(mock.clone()), Arc::clone(&gate)).unwrap();
    (runtime, gate)
}

impl Driver for MockDriver {
    fn version(&self) -> DriverResult<Vec<u8>> {
        let mut state = self.lock();
        state.calls.push(Call::Version);
        state.version.clone()
    }

    fn initialize(&self) -> DriverResult<()> {
        let mut state = self.lock();
        state.calls.push(Call::Initialize);
        pop_or(&mut state.initialize, Ok(()))
    }

    fn finalize(&self) -> DriverResult<()> {
        let mut state = self.lock();
        state.calls.push(Call::Finalize);
        pop_or(&mut state.finalize, Ok(()))
    }

    fn device_number(&self) -> DriverResult<u32> {
        let mut state = self.lock();
        state.calls.push(Call::DeviceNumber);
        pop_or(&mut state.device_number, Ok(0))
    }

    fn device_list(&self, capacity: usize) -> DriverResult<DeviceListAttempt> {
        let mut state = self.lock();
        state.calls.push(Call::DeviceList);
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
        Ok(())
    }

    fn open_by_ip(&self, _: &CStr) -> DriverResult<Handle> {
        self.open(Call::OpenByIp)
    }

    fn open_by_serial(&self, _: &CStr) -> DriverResult<Handle> {
        self.open(Call::OpenBySerial)
    }

    fn close(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.lock();
        state.calls.push(Call::Close);
        pop_or(&mut state.close, Ok(()))
    }

    fn start(&self, _: Handle) -> DriverResult<()> {
        self.record(Call::Start);
        Ok(())
    }

    fn stop(&self, _: Handle) -> DriverResult<()> {
        let mut state = self.lock();
        state.calls.push(Call::Stop);
        pop_or(&mut state.stop, Ok(()))
    }

    fn soft_trigger(&self, _: Handle) -> DriverResult<()> {
        self.record(Call::SoftTrigger);
        Ok(())
    }

    fn clear_buffer(&self, _: Handle) -> DriverResult<()> {
        self.record(Call::ClearBuffer);
        Ok(())
    }

    fn get_image(&self, _: Handle, timeout_ms: u32) -> DriverResult<FrameRecord> {
        let mut state = self.lock();
        state.calls.push(Call::GetImage);
        state.image_timeouts.push(timeout_ms);
        pop_or(
            &mut state.get_image,
            Err(DriverError::Status(0x8006_0006_u32 as i32)),
        )
    }

    fn register_image_callback(&self, _: Handle, _: CallbackCookie) -> DriverResult<()> {
        self.record(Call::RegisterImageCallback);
        Ok(())
    }

    fn register_exception_callback(&self, _: Handle, _: CallbackCookie) -> DriverResult<()> {
        self.record(Call::RegisterExceptionCallback);
        Ok(())
    }

    fn get_parameter(&self, _: Handle, _: &CStr) -> DriverResult<ParameterRecord> {
        Ok(ParameterRecord::Bool(false))
    }

    fn set_parameter(&self, _: Handle, _: &CStr, _: &ParameterValueRecord) -> DriverResult<()> {
        Ok(())
    }

    fn execute(&self, _: Handle, _: &CStr) -> DriverResult<()> {
        Ok(())
    }

    fn file_access_read(
        &self,
        _: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        self.record_file_call(Call::FileRead, user_file_name, device_file_name);
        Ok(())
    }

    fn file_access_write(
        &self,
        _: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        self.record_file_call(Call::FileWrite, user_file_name, device_file_name);
        Ok(())
    }

    fn file_access_progress(&self, _: Handle) -> DriverResult<FileProgressRaw> {
        let mut state = self.lock();
        state.calls.push(Call::FileProgress);
        pop_or(
            &mut state.file_progress,
            Ok(FileProgressRaw {
                completed: 0,
                total: 0,
            }),
        )
    }

    fn map_depth_to_point_cloud(&self, _: ImageInput<'_>) -> DriverResult<FrameRecord> {
        unsupported()
    }

    fn map_depth_to_point_cloud_round(&self, _: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        unsupported()
    }

    fn convert_image(&self, _: ImageInput<'_>, _: ImageTypeRecord) -> DriverResult<FrameRecord> {
        unsupported()
    }

    fn mosaic_depth(&self, _: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        unsupported()
    }

    fn save_image(
        &self,
        _: ImageInput<'_>,
        _: ImageFileFormatRecord,
        _: &CStr,
    ) -> DriverResult<()> {
        Ok(())
    }

    #[cfg(feature = "display-windows")]
    fn display_image(
        &self,
        _: ImageInput<'_>,
        _: NonZeroIsize,
        _: DisplayRangeRecord,
    ) -> DriverResult<()> {
        Ok(())
    }
}

impl MockDriver {
    fn record_file_call(&self, direction: Call, user: &CStr, device: &CStr) {
        let mut state = self.lock();
        state.calls.push(direction);
        state.file_calls.push(FileCall {
            direction,
            user_file_name: user.to_bytes().to_vec(),
            device_file_name: device.to_bytes().to_vec(),
        });
    }
}

fn pop_or<T>(queue: &mut VecDeque<DriverResult<T>>, default: DriverResult<T>) -> DriverResult<T> {
    queue.pop_front().unwrap_or(default)
}

fn unsupported<T>() -> DriverResult<T> {
    Err(DriverError::Status(0x8006_0001_u32 as i32))
}

fn mock_handle(address: usize) -> Handle {
    Handle::from_ptr(address as *mut std::ffi::c_void).unwrap()
}
