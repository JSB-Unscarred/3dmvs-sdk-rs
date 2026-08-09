#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ffi::CString;
use std::net::Ipv4Addr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::device::{DeviceRecord, IpConfigRaw, IpConfiguration};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{Driver, DriverError, DriverResult, Handle};
use crate::error::{ContractViolation, Error, InvalidInput, Operation};
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::opened_device::Device;

const AUDITED_VERSION_TEXT: &[u8] = b"1.3.3.3";
const MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE_TEXT: &[u8] = b"1.3.4.0";
const AUDITED_VERSION: SdkVersion = SdkVersion::new(1, 3, 3, 3);
const MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE: SdkVersion = SdkVersion::new(1, 3, 4, 0);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SdkVersion([u32; 4]);

impl SdkVersion {
    const fn new(major: u32, minor: u32, patch: u32, build: u32) -> Self {
        Self([major, minor, patch, build])
    }
}

/// Process-wide owner for the current native LPSDK session.
enum ProcessSdkState {
    Fresh,
    Active(Arc<RuntimeCore>),
}

pub(crate) struct Gate {
    // Active owns the core so dropping a user-facing Runtime token cannot finalize the SDK.
    state: Mutex<ProcessSdkState>,
}

impl Gate {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(ProcessSdkState::Fresh),
        }
    }

    fn lock(&self) -> MutexGuard<'_, ProcessSdkState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                // 生产路径持锁期间不执行用户代码；恢复锁即可继续使用已记录的状态。
                self.state.clear_poison();
                poisoned.into_inner()
            }
        }
    }
}

/// Owned native session shared by SDK tokens and devices.
pub(crate) struct RuntimeCore {
    driver: Box<dyn Driver>,
    version: Vec<u8>,
    handles: Mutex<usize>,
}

impl RuntimeCore {
    /// Calls a native operation while its caller supplies the required lifecycle guard or handle.
    pub(crate) fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        call(self.driver.as_ref()).map_err(|error| map_driver_error(operation, error))
    }

    fn lock_handles(&self) -> MutexGuard<'_, usize> {
        match self.handles.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                // handle 的增减只由 Device owner 驱动，恢复锁后沿用现有计数。
                self.handles.clear_poison();
                poisoned.into_inner()
            }
        }
    }

    pub(crate) fn open_handle(
        &self,
        operation: Operation,
        open: impl FnOnce(&dyn Driver) -> DriverResult<Handle>,
    ) -> Result<Handle, Error> {
        let mut live_handles = self.lock_handles();
        let handle =
            open(self.driver.as_ref()).map_err(|error| map_driver_error(operation, error))?;
        *live_handles += 1;
        Ok(handle)
    }

    pub(crate) fn cleanup_close_handle(&self, handle: Handle) -> Result<(), Error> {
        const OPERATION: Operation = Operation::CloseDevice;

        let mut live_handles = self.lock_handles();
        self.driver
            .close(handle)
            .map_err(|error| map_driver_error(OPERATION, error))?;
        *live_handles = live_handles
            .checked_sub(1)
            .expect("a Device closes its owned handle at most once");
        Ok(())
    }

    pub(crate) fn parameter_key(operation: Operation, bytes: &[u8]) -> Result<CString, Error> {
        CString::new(bytes).map_err(|_| Error::InvalidInput {
            operation,
            kind: InvalidInput::InteriorNul,
        })
    }
}

#[derive(Clone)]
/// Control token for the process-wide native session.
///
/// Dropping a token leaves the session active; explicit `shutdown` performs Finalize after every
/// device closes.
pub struct Runtime {
    core: Arc<RuntimeCore>,
    gate: Arc<Gate>,
}

impl Runtime {
    pub fn initialize() -> Result<Self, Error> {
        Self::initialize_native()
    }

    fn initialize_native() -> Result<Self, Error> {
        #[cfg(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        ))]
        {
            static GATE: OnceLock<Arc<Gate>> = OnceLock::new();
            let gate = Arc::clone(GATE.get_or_init(|| Arc::new(Gate::new())));
            Self::initialize_driver(Box::new(crate::ffi::NativeDriver), gate)
        }

        #[cfg(not(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )))]
        {
            let _ = OnceLock::<Arc<Gate>>::new();
            Err(Error::UnsupportedPlatform)
        }
    }

    #[cfg(test)]
    pub(crate) fn initialize_with(driver: Box<dyn Driver>, gate: Arc<Gate>) -> Result<Self, Error> {
        Self::initialize_driver(driver, gate)
    }

    fn initialize_driver(driver: Box<dyn Driver>, gate: Arc<Gate>) -> Result<Self, Error> {
        let mut state = gate.lock();
        match &*state {
            ProcessSdkState::Fresh => {}
            ProcessSdkState::Active(core) => {
                validate_sdk_version(&core.version)?;
                return Ok(Self {
                    core: Arc::clone(core),
                    gate: Arc::clone(&gate),
                });
            }
        }

        let version = match driver.version() {
            Ok(version) => version,
            Err(error) => return Err(map_driver_error(Operation::GetVersion, error)),
        };
        validate_sdk_version(&version)?;

        driver
            .initialize()
            .map_err(|error| map_driver_error(Operation::Initialize, error))?;

        let core = Arc::new(RuntimeCore {
            driver,
            version,
            handles: Mutex::new(0),
        });
        *state = ProcessSdkState::Active(Arc::clone(&core));
        drop(state);
        Ok(Self { core, gate })
    }

    pub fn version_bytes(&self) -> &[u8] {
        &self.core.version
    }

    pub fn device_count_hint(&self) -> Result<u32, Error> {
        self.call(Operation::GetDeviceNumber, |driver| driver.device_number())
    }

    pub fn devices(&self) -> Result<Vec<DeviceRecord>, Error> {
        let count = self.device_count_hint()?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let capacity = usize::try_from(count).expect("u32 fits usize on supported targets");
        let list = self.call(Operation::GetDeviceList, |driver| {
            driver.device_list(capacity)
        })?;
        let reported = usize::try_from(list.reported).expect("u32 fits usize on supported targets");
        if list.records.len() != reported {
            return Err(Error::ContractViolation {
                operation: Operation::GetDeviceList,
                kind: ContractViolation::DeviceListCountMismatch {
                    reported: list.reported,
                    returned: list.records.len(),
                },
            });
        }
        Ok(list.records.into_iter().map(DeviceRecord::from).collect())
    }

    pub fn set_ip_config(
        &self,
        serial_number: &[u8],
        configuration: &IpConfiguration,
    ) -> Result<(), Error> {
        let serial = validated_c_string(Operation::SetIpConfig, serial_number, 16)?;
        let raw = IpConfigRaw::from(configuration);
        self.call(Operation::SetIpConfig, |driver| {
            driver.set_ip_config(&serial, &raw)
        })
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device, Error> {
        let address = CString::new(address.to_string()).expect("an IPv4 address contains no NUL");
        let handle = self.open(Operation::OpenDeviceByIp, |driver| {
            driver.open_by_ip(&address)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    pub fn open_by_serial(&self, serial_number: &[u8]) -> Result<Device, Error> {
        let serial = validated_c_string(Operation::OpenDeviceBySn, serial_number, 16)?;
        let handle = self.open(Operation::OpenDeviceBySn, |driver| {
            driver.open_by_serial(&serial)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    pub fn map_depth_to_point_cloud(&self, input: ImageInput<'_>) -> Result<FrameRecord, Error> {
        self.call(Operation::MapDepthToPointCloud, |driver| {
            driver.map_depth_to_point_cloud(input)
        })
    }

    pub fn map_depth_to_point_cloud_round(
        &self,
        inputs: &[ImageInput<'_>],
    ) -> Result<FrameRecord, Error> {
        self.call(Operation::MapDepthToPointCloudRound, |driver| {
            driver.map_depth_to_point_cloud_round(inputs)
        })
    }

    pub fn convert_image(
        &self,
        input: ImageInput<'_>,
        target: ImageTypeRecord,
    ) -> Result<FrameRecord, Error> {
        self.call(Operation::ImageConvert, |driver| {
            driver.convert_image(input, target)
        })
    }

    pub fn mosaic_depth(&self, inputs: &[ImageInput<'_>]) -> Result<FrameRecord, Error> {
        self.call(Operation::DepthMosaic, |driver| driver.mosaic_depth(inputs))
    }

    pub fn save_image(
        &self,
        input: ImageInput<'_>,
        format: ImageFileFormatRecord,
        file_name: &[u8],
    ) -> Result<(), Error> {
        let file_name = validated_c_string(Operation::SaveImage, file_name, u32::MAX as usize)?;
        self.call(Operation::SaveImage, |driver| {
            driver.save_image(input, format, &file_name)
        })
    }

    #[cfg(feature = "display-windows")]
    pub fn display_image(
        &self,
        input: ImageInput<'_>,
        window: NonZeroIsize,
        range: DisplayRangeRecord,
    ) -> Result<(), Error> {
        self.call(Operation::DisplayImage, |driver| {
            driver.display_image(input, window, range)
        })
    }

    /// Finalizes the native session after every owned device closes.
    ///
    /// Repeating the call while the process remains Fresh succeeds; a stale token cannot finalize
    /// a newer session.
    pub fn shutdown(&self) -> Result<(), Error> {
        let mut process = self.gate.lock();
        match &*process {
            ProcessSdkState::Active(core) if Arc::ptr_eq(core, &self.core) => {}
            ProcessSdkState::Fresh => return Ok(()),
            ProcessSdkState::Active(_) => return Err(Error::RuntimeInactive),
        }

        let handles = self.core.lock_handles();
        if *handles != 0 {
            return Err(Error::UnclosedDevices {
                live_handles: *handles,
            });
        }

        let result = self
            .core
            .driver
            .finalize()
            .map_err(|error| map_driver_error(Operation::Finalize, error));
        drop(handles);
        match result {
            Ok(()) => {
                *process = ProcessSdkState::Fresh;
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn open(
        &self,
        operation: Operation,
        open: impl FnOnce(&dyn Driver) -> DriverResult<Handle>,
    ) -> Result<Handle, Error> {
        let _active = self.active_state()?;
        self.core.open_handle(operation, open)
    }

    fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let _active = self.active_state()?;
        self.core.call(operation, call)
    }

    /// Holds the process gate across a short native call and rejects stale SDK tokens.
    fn active_state(&self) -> Result<MutexGuard<'_, ProcessSdkState>, Error> {
        let state = self.gate.lock();
        match &*state {
            ProcessSdkState::Active(core) if Arc::ptr_eq(core, &self.core) => Ok(state),
            ProcessSdkState::Active(_) | ProcessSdkState::Fresh => Err(Error::RuntimeInactive),
        }
    }
}

fn validate_sdk_version(bytes: &[u8]) -> Result<(), Error> {
    if parse_sdk_version(bytes).is_some_and(|version| {
        (AUDITED_VERSION..MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE).contains(&version)
    }) {
        Ok(())
    } else {
        Err(Error::IncompatibleSdkVersion {
            minimum: AUDITED_VERSION_TEXT,
            maximum_exclusive: MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE_TEXT,
            actual: bytes.to_vec(),
        })
    }
}

fn parse_sdk_version(bytes: &[u8]) -> Option<SdkVersion> {
    let mut components = std::str::from_utf8(bytes).ok()?.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components.next()?.parse().ok()?;
    let build = components.next()?.parse().ok()?;
    if components.next().is_some() {
        return None;
    }
    Some(SdkVersion::new(major, minor, patch, build))
}

fn map_driver_error(operation: Operation, error: DriverError) -> Error {
    match error {
        DriverError::Status(status) => Error::Sdk { operation, status },
        DriverError::InvalidInput(kind) => Error::InvalidInput { operation, kind },
        DriverError::Contract(kind) => Error::ContractViolation { operation, kind },
        DriverError::Allocation { requested } => Error::AllocationFailed {
            operation,
            requested,
        },
    }
}

fn validated_c_string(
    operation: Operation,
    bytes: &[u8],
    maximum: usize,
) -> Result<CString, Error> {
    if bytes.is_empty() {
        return Err(Error::InvalidInput {
            operation,
            kind: InvalidInput::Empty,
        });
    }
    if bytes.len() > maximum {
        return Err(Error::InvalidInput {
            operation,
            kind: InvalidInput::TooLong {
                actual: bytes.len(),
                maximum,
            },
        });
    }
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        operation,
        kind: InvalidInput::InteriorNul,
    })
}
