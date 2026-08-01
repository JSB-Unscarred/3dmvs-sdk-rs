use std::collections::{HashMap, VecDeque};
use std::ffi::CStr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
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

#[derive(Clone)]
pub(crate) struct MockDriver {
    shared: Arc<Mutex<MockState>>,
    call_probe: Arc<CallProbe>,
}

/// The post-FFI operation surface exercised by [`MockDriver`].
///
/// This is deliberately a Driver-layer ledger: injecting one of these failures tests Runtime and
/// Device behavior after the native boundary has already converted raw outputs. Raw pointer,
/// union, and status-before-output behavior remains covered by the focused `ffi` conversion tests.
#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub(crate) enum FfiOp {
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
    #[cfg(feature = "display-windows")]
    DisplayImage,
}

impl FfiOp {
    pub(crate) const ALL: &'static [Self] = &[
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
        #[cfg(feature = "display-windows")]
        Self::DisplayImage,
    ];

    pub(crate) const fn sdk_name(self) -> &'static str {
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
            #[cfg(feature = "display-windows")]
            Self::DisplayImage => "MV3D_LP_DisplayImage",
        }
    }

    pub(crate) const fn driver_method(self) -> &'static str {
        match self {
            Self::GetVersion => "version",
            Self::Initialize => "initialize",
            Self::Finalize => "finalize",
            Self::GetDeviceNumber => "device_number",
            Self::GetDeviceList => "device_list",
            Self::SetIpConfig => "set_ip_config",
            Self::OpenDeviceByIp => "open_by_ip",
            Self::OpenDeviceBySerial => "open_by_serial",
            Self::CloseDevice => "close",
            Self::StartMeasure => "start",
            Self::StopMeasure => "stop",
            Self::SoftTrigger => "soft_trigger",
            Self::ClearDataBuffer => "clear_buffer",
            Self::GetImage => "get_image",
            Self::RegisterImageCallback => "register_image_callback",
            Self::RegisterExceptionCallback => "register_exception_callback",
            Self::GetParameter => "get_parameter",
            Self::SetParameter => "set_parameter",
            Self::Execute => "execute",
            Self::FileAccessRead => "file_access_read",
            Self::FileAccessWrite => "file_access_write",
            Self::GetFileAccessProgress => "file_access_progress",
            Self::MapDepthToPointCloud => "map_depth_to_point_cloud",
            Self::MapDepthToPointCloudRound => "map_depth_to_point_cloud_round",
            Self::ImageConvert => "convert_image",
            Self::DepthMosaic => "mosaic_depth",
            Self::SaveImage => "save_image",
            #[cfg(feature = "display-windows")]
            Self::DisplayImage => "display_image",
        }
    }
}

#[derive(Default)]
struct CallProbe {
    in_flight: AtomicUsize,
    maximum_in_flight: AtomicUsize,
    hooks: Mutex<HashMap<FfiOp, CallHook>>,
}

struct CallHook {
    remaining: usize,
    action: Arc<dyn Fn() + Send + Sync>,
}

struct CallGuard<'a> {
    probe: &'a CallProbe,
    operation: FfiOp,
}

impl CallProbe {
    fn enter(&self, operation: FfiOp) -> CallGuard<'_> {
        let previous = self.in_flight.fetch_add(1, Ordering::SeqCst);
        let guard = CallGuard {
            probe: self,
            operation,
        };
        let current = previous
            .checked_add(1)
            .expect("mock call probe counter overflowed");
        self.maximum_in_flight.fetch_max(current, Ordering::SeqCst);

        let action = {
            let mut hooks = self.hooks();
            match hooks.remove(&operation) {
                Some(mut hook) => {
                    hook.remaining -= 1;
                    let action = Arc::clone(&hook.action);
                    if hook.remaining != 0 {
                        hooks.insert(operation, hook);
                    }
                    Some(action)
                }
                None => None,
            }
        };
        if let Some(action) = action {
            action();
        }

        guard
    }

    fn hook_next_calls(
        &self,
        operation: FfiOp,
        callers: usize,
        action: Arc<dyn Fn() + Send + Sync>,
    ) {
        assert!(callers != 0, "a mock call hook needs at least one caller");
        let mut hooks = self.hooks();
        match hooks.entry(operation) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(CallHook {
                    remaining: callers,
                    action,
                });
            }
            std::collections::hash_map::Entry::Occupied(_) => {
                panic!("only one mock call hook may be active for {operation:?}")
            }
        }
    }

    fn hooks(&self) -> MutexGuard<'_, HashMap<FfiOp, CallHook>> {
        match self.hooks.lock() {
            Ok(hooks) => hooks,
            Err(poisoned) => {
                let hooks = poisoned.into_inner();
                self.hooks.clear_poison();
                hooks
            }
        }
    }
}

impl CallGuard<'_> {
    fn operation(&self) -> FfiOp {
        self.operation
    }
}

impl Drop for CallGuard<'_> {
    fn drop(&mut self) {
        self.probe.in_flight.fetch_sub(1, Ordering::SeqCst);
    }
}

#[derive(Clone, Copy, Debug)]
struct MockHandleId(NonZeroUsize);

impl MockHandleId {
    fn new(address: usize) -> Self {
        Self(NonZeroUsize::new(address).expect("a mock handle address must be non-zero"))
    }

    fn address(self) -> usize {
        self.0.get()
    }

    fn into_handle(self) -> Handle {
        mock_handle(self.address())
    }
}

#[derive(Clone, Debug)]
struct OpenReply {
    handle: Option<MockHandleId>,
    result: DriverResult<()>,
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
    poison_recoveries: usize,
    log: Vec<&'static str>,
    operations: Vec<FfiOp>,
    injected_failures: HashMap<FfiOp, VecDeque<DriverError>>,
    capacities: Vec<usize>,
    version: DriverResult<Vec<u8>>,
    initialize: VecDeque<DriverResult<()>>,
    finalize: VecDeque<DriverResult<()>>,
    device_number: VecDeque<DriverResult<u32>>,
    device_list: VecDeque<DriverResult<DeviceListAttempt>>,
    next_handle: usize,
    open_by_ip: VecDeque<OpenReply>,
    open_by_serial: VecDeque<OpenReply>,
    opened_handles: Vec<(FfiOp, usize)>,
    closed_handles: Vec<usize>,
    close: VecDeque<DriverResult<()>>,
    start: VecDeque<DriverResult<()>>,
    stop: VecDeque<DriverResult<()>>,
    soft_trigger: VecDeque<DriverResult<()>>,
    clear_buffer: VecDeque<DriverResult<()>>,
    get_image: VecDeque<DriverResult<FrameRecord>>,
    image_timeouts: Vec<u32>,
    register_image_callback: VecDeque<DriverResult<()>>,
    register_exception_callback: VecDeque<DriverResult<()>>,
    image_callback_cookies: Vec<CallbackCookie>,
    exception_callback_cookies: Vec<CallbackCookie>,
    get_parameter: VecDeque<DriverResult<ParameterRecord>>,
    set_ip_config: VecDeque<DriverResult<()>>,
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
            shared: Arc::new(Mutex::new(MockState {
                poison_recoveries: 0,
                log: Vec::new(),
                operations: Vec::new(),
                injected_failures: HashMap::new(),
                capacities: Vec::new(),
                version: Ok(b"1.3.3.3".to_vec()),
                initialize: VecDeque::new(),
                finalize: VecDeque::new(),
                device_number: VecDeque::new(),
                device_list: VecDeque::new(),
                next_handle: 1,
                open_by_ip: VecDeque::new(),
                open_by_serial: VecDeque::new(),
                opened_handles: Vec::new(),
                closed_handles: Vec::new(),
                close: VecDeque::new(),
                start: VecDeque::new(),
                stop: VecDeque::new(),
                soft_trigger: VecDeque::new(),
                clear_buffer: VecDeque::new(),
                get_image: VecDeque::new(),
                image_timeouts: Vec::new(),
                register_image_callback: VecDeque::new(),
                register_exception_callback: VecDeque::new(),
                image_callback_cookies: Vec::new(),
                exception_callback_cookies: Vec::new(),
                get_parameter: VecDeque::new(),
                set_ip_config: VecDeque::new(),
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
            call_probe: Arc::new(CallProbe::default()),
        }
    }

    pub(crate) fn set_version(&self, result: DriverResult<Vec<u8>>) {
        self.state().version = result;
    }

    /// Injects a Driver-layer failure for the next invocation of `operation`.
    ///
    /// Unlike the typed result queues below, this entry point is shared by every [`FfiOp`]. It is
    /// the coverage backstop that prevents a newly added Driver method from silently remaining
    /// non-injectable in the hardening suite.
    pub(crate) fn fail_next(&self, operation: FfiOp, error: DriverError) {
        self.state()
            .injected_failures
            .entry(operation)
            .or_default()
            .push_back(error);
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

    pub(crate) fn configure_open_by_ip(&self, handle: Option<usize>, result: DriverResult<()>) {
        self.state().open_by_ip.push_back(OpenReply {
            handle: handle.map(MockHandleId::new),
            result,
        });
    }

    pub(crate) fn configure_open_by_serial(&self, handle: Option<usize>, result: DriverResult<()>) {
        self.state().open_by_serial.push_back(OpenReply {
            handle: handle.map(MockHandleId::new),
            result,
        });
    }

    pub(crate) fn push_close(&self, result: DriverResult<()>) {
        self.state().close.push_back(result);
    }

    pub(crate) fn set_close_entered(&self, entered: Arc<AtomicBool>) {
        self.hook_next_calls(
            FfiOp::CloseDevice,
            1,
            Arc::new(move || entered.store(true, Ordering::SeqCst)),
        );
    }

    pub(crate) fn push_start(&self, result: DriverResult<()>) {
        self.state().start.push_back(result);
    }

    pub(crate) fn push_stop(&self, result: DriverResult<()>) {
        self.state().stop.push_back(result);
    }

    pub(crate) fn set_stop_entered(&self, entered: Arc<AtomicBool>) {
        self.hook_next_calls(
            FfiOp::StopMeasure,
            1,
            Arc::new(move || entered.store(true, Ordering::SeqCst)),
        );
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

    pub(crate) fn push_set_ip_config(&self, result: DriverResult<()>) {
        self.state().set_ip_config.push_back(result);
    }

    pub(crate) fn push_set_parameter(&self, result: DriverResult<()>) {
        self.state().set_parameter.push_back(result);
    }

    pub(crate) fn push_execute(&self, result: DriverResult<()>) {
        self.state().execute.push_back(result);
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

    pub(crate) fn operations(&self) -> Vec<FfiOp> {
        self.state().operations.clone()
    }

    pub(crate) fn opened_handles(&self) -> Vec<(FfiOp, usize)> {
        self.state().opened_handles.clone()
    }

    pub(crate) fn closed_handles(&self) -> Vec<usize> {
        self.state().closed_handles.clone()
    }

    pub(crate) fn assert_no_pending_failures(&self) {
        let pending = self
            .state()
            .injected_failures
            .iter()
            .filter_map(|(operation, failures)| {
                (!failures.is_empty()).then_some((*operation, failures.len()))
            })
            .collect::<Vec<_>>();
        assert!(
            pending.is_empty(),
            "unconsumed Driver-layer failure injections: {pending:?}"
        );
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

    pub(crate) fn in_flight(&self) -> usize {
        self.call_probe.in_flight.load(Ordering::SeqCst)
    }

    pub(crate) fn maximum_in_flight(&self) -> usize {
        self.call_probe.maximum_in_flight.load(Ordering::SeqCst)
    }

    pub(crate) fn hook_next_calls(
        &self,
        operation: FfiOp,
        callers: usize,
        action: Arc<dyn Fn() + Send + Sync>,
    ) {
        self.call_probe.hook_next_calls(operation, callers, action);
    }

    fn poison_recoveries(&self) -> usize {
        self.state().poison_recoveries
    }

    fn set_next_handle(&self, address: usize) {
        self.state().next_handle = address;
    }

    #[cfg(feature = "display-windows")]
    pub(crate) fn display_calls(&self) -> Vec<DisplayCall> {
        self.state().display_calls.clone()
    }

    fn state(&self) -> MutexGuard<'_, MockState> {
        match self.shared.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                state.poison_recoveries += 1;
                self.shared.clear_poison();
                state
            }
        }
    }

    fn enter(&self, operation: FfiOp) -> CallGuard<'_> {
        self.call_probe.enter(operation)
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
        let _call = self.enter(FfiOp::GetVersion);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        state.version.clone()
    }

    fn initialize(&self) -> DriverResult<()> {
        let _call = self.enter(FfiOp::Initialize);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.initialize, Ok(()))
    }

    fn finalize(&self) -> DriverResult<()> {
        let _call = self.enter(FfiOp::Finalize);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.finalize, Ok(()))
    }

    fn device_number(&self) -> DriverResult<u32> {
        let _call = self.enter(FfiOp::GetDeviceNumber);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.device_number, Ok(0))
    }

    fn device_list(&self, capacity: usize) -> DriverResult<DeviceListAttempt> {
        let _call = self.enter(FfiOp::GetDeviceList);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.capacities.push(capacity);
        return_injected_failure(&mut state, &_call)?;
        pop_or(
            &mut state.device_list,
            Ok(DeviceListAttempt {
                records: Vec::new(),
                reported: 0,
            }),
        )
    }

    fn set_ip_config(&self, _: &CStr, _: &IpConfigRaw) -> DriverResult<()> {
        let _call = self.enter(FfiOp::SetIpConfig);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.set_ip_config, Ok(()))
    }

    fn open_by_ip(&self, _: &CStr, handle: &mut Option<Handle>) -> DriverResult<()> {
        let _call = self.enter(FfiOp::OpenDeviceByIp);
        self.open(&_call, handle)
    }

    fn open_by_serial(&self, _: &CStr, handle: &mut Option<Handle>) -> DriverResult<()> {
        let _call = self.enter(FfiOp::OpenDeviceBySerial);
        self.open(&_call, handle)
    }

    fn close(&self, handle: Handle) -> DriverResult<()> {
        let _call = self.enter(FfiOp::CloseDevice);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.closed_handles.push(handle.as_ptr().addr());
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.close, Ok(()))
    }

    fn start(&self, _: Handle) -> DriverResult<()> {
        let _call = self.enter(FfiOp::StartMeasure);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.start, Ok(()))
    }

    fn stop(&self, _: Handle) -> DriverResult<()> {
        let _call = self.enter(FfiOp::StopMeasure);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.stop, Ok(()))
    }

    fn soft_trigger(&self, _: Handle) -> DriverResult<()> {
        let _call = self.enter(FfiOp::SoftTrigger);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.soft_trigger, Ok(()))
    }

    fn clear_buffer(&self, _: Handle) -> DriverResult<()> {
        let _call = self.enter(FfiOp::ClearDataBuffer);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.clear_buffer, Ok(()))
    }

    fn get_image(&self, _: Handle, timeout_ms: u32) -> DriverResult<FrameRecord> {
        let _call = self.enter(FfiOp::GetImage);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.image_timeouts.push(timeout_ms);
        return_injected_failure(&mut state, &_call)?;
        pop_or(
            &mut state.get_image,
            Err(crate::driver::DriverError::Status(0x8006_0006_u32 as i32)),
        )
    }

    fn register_image_callback(&self, _: Handle, cookie: CallbackCookie) -> DriverResult<()> {
        let _call = self.enter(FfiOp::RegisterImageCallback);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.image_callback_cookies.push(cookie);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.register_image_callback, Ok(()))
    }

    fn register_exception_callback(&self, _: Handle, cookie: CallbackCookie) -> DriverResult<()> {
        let _call = self.enter(FfiOp::RegisterExceptionCallback);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.exception_callback_cookies.push(cookie);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.register_exception_callback, Ok(()))
    }

    fn get_parameter(&self, _: Handle, _: &CStr) -> DriverResult<ParameterRecord> {
        let _call = self.enter(FfiOp::GetParameter);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.get_parameter, Ok(ParameterRecord::Bool(false)))
    }

    fn set_parameter(&self, _: Handle, _: &CStr, _: &ParameterValueRecord) -> DriverResult<()> {
        let _call = self.enter(FfiOp::SetParameter);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.set_parameter, Ok(()))
    }

    fn execute(&self, _: Handle, _: &CStr) -> DriverResult<()> {
        let _call = self.enter(FfiOp::Execute);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.execute, Ok(()))
    }

    fn file_access_read(
        &self,
        _: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        let _call = self.enter(FfiOp::FileAccessRead);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.file_access_calls.push(FileAccessCall {
            operation: "file_access_read",
            user_file_name: user_file_name.to_bytes().to_vec(),
            device_file_name: device_file_name.to_bytes().to_vec(),
            user_file_name_address: user_file_name.as_ptr() as usize,
            device_file_name_address: device_file_name.as_ptr() as usize,
        });
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.file_access_read, Ok(()))
    }

    fn file_access_write(
        &self,
        _: Handle,
        user_file_name: &CStr,
        device_file_name: &CStr,
    ) -> DriverResult<()> {
        let _call = self.enter(FfiOp::FileAccessWrite);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.file_access_calls.push(FileAccessCall {
            operation: "file_access_write",
            user_file_name: user_file_name.to_bytes().to_vec(),
            device_file_name: device_file_name.to_bytes().to_vec(),
            user_file_name_address: user_file_name.as_ptr() as usize,
            device_file_name_address: device_file_name.as_ptr() as usize,
        });
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.file_access_write, Ok(()))
    }

    fn file_access_progress(&self, _: Handle) -> DriverResult<FileProgressRaw> {
        let _call = self.enter(FfiOp::GetFileAccessProgress);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(
            &mut state.file_access_progress,
            Ok(FileProgressRaw {
                completed: 0,
                total: 0,
            }),
        )
    }

    fn map_depth_to_point_cloud(&self, _: ImageInput<'_>) -> DriverResult<FrameRecord> {
        let _call = self.enter(FfiOp::MapDepthToPointCloud);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(
            &mut state.map_depth_to_point_cloud,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn map_depth_to_point_cloud_round(&self, _: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        let _call = self.enter(FfiOp::MapDepthToPointCloudRound);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(
            &mut state.map_depth_to_point_cloud_round,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn convert_image(&self, _: ImageInput<'_>, _: ImageTypeRecord) -> DriverResult<FrameRecord> {
        let _call = self.enter(FfiOp::ImageConvert);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(
            &mut state.convert_image,
            Err(crate::driver::DriverError::Status(0x8006_0001_u32 as i32)),
        )
    }

    fn mosaic_depth(&self, _: &[ImageInput<'_>]) -> DriverResult<FrameRecord> {
        let _call = self.enter(FfiOp::DepthMosaic);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
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
        let _call = self.enter(FfiOp::SaveImage);
        let mut state = self.state();
        record_call(&mut state, &_call);
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.save_image, Ok(()))
    }

    #[cfg(feature = "display-windows")]
    fn display_image(
        &self,
        _: ImageInput<'_>,
        window: NonZeroIsize,
        range: DisplayRangeRecord,
    ) -> DriverResult<()> {
        let _call = self.enter(FfiOp::DisplayImage);
        let mut state = self.state();
        record_call(&mut state, &_call);
        state.display_calls.push(DisplayCall { window, range });
        return_injected_failure(&mut state, &_call)?;
        pop_or(&mut state.display_image, Ok(()))
    }
}

impl MockDriver {
    fn open(&self, call: &CallGuard<'_>, output: &mut Option<Handle>) -> DriverResult<()> {
        let operation = call.operation();
        let (handle, result) = {
            let mut state = self.state();
            record_call(&mut state, call);
            return_injected_failure(&mut state, call)?;

            let configured = match operation {
                FfiOp::OpenDeviceByIp => state.open_by_ip.pop_front(),
                FfiOp::OpenDeviceBySerial => state.open_by_serial.pop_front(),
                _ => unreachable!("only open operations use MockDriver::open"),
            };
            let reply = configured.unwrap_or_else(|| {
                let handle = MockHandleId::new(state.next_handle);
                state.next_handle = state
                    .next_handle
                    .checked_add(1)
                    .expect("mock handle sequence exhausted");
                OpenReply {
                    handle: Some(handle),
                    result: Ok(()),
                }
            });
            if let Some(handle) = reply.handle {
                state.opened_handles.push((operation, handle.address()));
            }
            (reply.handle, reply.result)
        };

        *output = handle.map(MockHandleId::into_handle);
        result
    }
}

fn record_call(state: &mut MockState, call: &CallGuard<'_>) {
    let operation = call.operation();
    state.log.push(operation.driver_method());
    state.operations.push(operation);
}

fn return_injected_failure(state: &mut MockState, call: &CallGuard<'_>) -> DriverResult<()> {
    let operation = call.operation();
    let error = state
        .injected_failures
        .get_mut(&operation)
        .and_then(VecDeque::pop_front);
    match error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn pop_or<T>(queue: &mut VecDeque<DriverResult<T>>, default: DriverResult<T>) -> DriverResult<T> {
    queue.pop_front().unwrap_or(default)
}

pub(crate) fn mock_handle(address: usize) -> Handle {
    Handle::from_ptr(address as *mut std::ffi::c_void)
        .expect("a mock handle address must be non-zero")
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;
    use std::time::Duration;

    use super::*;

    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}

    #[derive(Default)]
    struct Rendezvous {
        arrived: Mutex<usize>,
        ready: Condvar,
    }

    impl Rendezvous {
        fn wait_for_two(&self) {
            let mut arrived = self.arrived.lock().unwrap();
            *arrived += 1;
            self.ready.notify_all();
            let (arrived, timeout) = self
                .ready
                .wait_timeout_while(arrived, Duration::from_secs(5), |arrived| *arrived < 2)
                .unwrap();
            assert!(
                !timeout.timed_out() || *arrived >= 2,
                "mock calls failed to rendezvous before the ledger lock"
            );
        }
    }

    #[test]
    fn shared_state_and_driver_have_the_required_auto_traits() {
        assert_send::<MockState>();
        assert_send::<MockDriver>();
        assert_sync::<MockDriver>();
    }

    #[test]
    fn probe_observes_concurrency_and_consumes_one_failure_exactly_once() {
        let mock = Arc::new(MockDriver::new());
        let injected = DriverError::Status(0x8006_0001_u32 as i32);
        mock.fail_next(FfiOp::GetVersion, injected.clone());
        let rendezvous = Arc::new(Rendezvous::default());
        let hook_rendezvous = Arc::clone(&rendezvous);
        mock.hook_next_calls(
            FfiOp::GetVersion,
            2,
            Arc::new(move || hook_rendezvous.wait_for_two()),
        );

        let first_mock = Arc::clone(&mock);
        let first = thread::spawn(move || first_mock.version());
        let second_mock = Arc::clone(&mock);
        let second = thread::spawn(move || second_mock.version());

        let outcomes = [first.join().unwrap(), second.join().unwrap()];
        assert!(outcomes.contains(&Ok(b"1.3.3.3".to_vec())));
        assert!(outcomes.contains(&Err(injected)));
        assert!(mock.maximum_in_flight() >= 2);
        assert_eq!(mock.in_flight(), 0);
        mock.assert_no_pending_failures();
        assert_eq!(
            mock.operations(),
            vec![FfiOp::GetVersion, FfiOp::GetVersion]
        );
    }

    #[test]
    fn probe_balances_a_panic_and_the_poisoned_ledger_recovers() {
        let mock = MockDriver::new();
        mock.set_next_handle(usize::MAX);
        let ip = c"127.0.0.1";

        let panic = catch_unwind(AssertUnwindSafe(|| {
            let mut output = None;
            let _ = mock.open_by_ip(ip, &mut output);
        }));

        assert!(panic.is_err());
        assert_eq!(mock.in_flight(), 0);
        assert_eq!(mock.poison_recoveries(), 1);
        assert_eq!(mock.version(), Ok(b"1.3.3.3".to_vec()));
        assert_eq!(mock.poison_recoveries(), 1);
        assert_eq!(
            mock.operations(),
            vec![FfiOp::OpenDeviceByIp, FfiOp::GetVersion]
        );
    }
}
