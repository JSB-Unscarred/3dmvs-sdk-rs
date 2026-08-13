#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ffi::CString;
use std::net::Ipv4Addr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::device::{DeviceRecord, IpConfigRaw, IpConfiguration};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{Driver, DriverError, DriverResult, Handle};
use crate::error::{ContractViolation, Error, InvalidInput, Operation};
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::opened_device::Device;

/// Process-wide owner for the current native LPSDK session.
enum ProcessSdkState {
    Fresh,
    Active(Arc<RuntimeCore>),
}

pub(crate) struct Gate {
    // Active owns the core so dropping a user-facing Runtime token cannot finalize the SDK.
    state: RwLock<ProcessSdkState>,
}

impl Gate {
    pub(crate) fn new() -> Self {
        Self {
            state: RwLock::new(ProcessSdkState::Fresh),
        }
    }

    fn read(&self) -> RwLockReadGuard<'_, ProcessSdkState> {
        match self.state.read() {
            Ok(state) => state,
            Err(poisoned) => {
                self.state.clear_poison();
                poisoned.into_inner()
            }
        }
    }

    fn write(&self) -> RwLockWriteGuard<'_, ProcessSdkState> {
        match self.state.write() {
            Ok(state) => state,
            Err(poisoned) => {
                self.state.clear_poison();
                poisoned.into_inner()
            }
        }
    }
}

/// Owned native session shared by SDK tokens and devices.
pub(crate) struct RuntimeCore {
    driver: Box<dyn Driver>,
    handles: Mutex<usize>,
    // 图像处理输出只在下一次处理调用前有效；同一 session 串行到 owned copy 完成。
    image_processing: Mutex<()>,
}

impl RuntimeCore {
    /// Calls a native operation while its caller supplies the required lifecycle guard or handle.
    pub(crate) fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let _image_processing = image_processing_guard(&self.image_processing, operation);
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
        // Close returning permanently consumes this owner even when the SDK reports an error.
        let result = self
            .driver
            .close(handle)
            .map_err(|error| map_driver_error(OPERATION, error));
        *live_handles = live_handles
            .checked_sub(1)
            .expect("a Device closes its owned handle at most once");
        result
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
    /// Reads the raw SDK version independently of the initialized session.
    pub fn version_bytes() -> Result<Vec<u8>, Error> {
        #[cfg(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        ))]
        {
            let driver = crate::ffi::NativeDriver;
            driver
                .version()
                .map_err(|error| map_driver_error(Operation::GetVersion, error))
        }

        #[cfg(not(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )))]
        Err(Error::UnsupportedPlatform)
    }

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

    fn initialize_driver(driver: Box<dyn Driver>, gate: Arc<Gate>) -> Result<Self, Error> {
        let mut state = gate.write();
        match &*state {
            ProcessSdkState::Fresh => {}
            ProcessSdkState::Active(core) => {
                return Ok(Self {
                    core: Arc::clone(core),
                    gate: Arc::clone(&gate),
                });
            }
        }

        driver
            .initialize()
            .map_err(|error| map_driver_error(Operation::Initialize, error))?;

        let core = Arc::new(RuntimeCore {
            driver,
            handles: Mutex::new(0),
            image_processing: Mutex::new(()),
        });
        *state = ProcessSdkState::Active(Arc::clone(&core));
        drop(state);
        Ok(Self { core, gate })
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
        let mut process = self.gate.write();
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

    /// Holds a shared session guard so Finalize cannot overlap a native call.
    fn active_state(&self) -> Result<RwLockReadGuard<'_, ProcessSdkState>, Error> {
        let state = self.gate.read();
        match &*state {
            ProcessSdkState::Active(core) if Arc::ptr_eq(core, &self.core) => Ok(state),
            ProcessSdkState::Active(_) | ProcessSdkState::Fresh => Err(Error::RuntimeInactive),
        }
    }
}

/// Serializes process-wide image helpers through the immediate SDK-output copy.
fn image_processing_guard(lock: &Mutex<()>, operation: Operation) -> Option<MutexGuard<'_, ()>> {
    operation_uses_image_processing_lock(operation).then(|| match lock.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            lock.clear_poison();
            poisoned.into_inner()
        }
    })
}

fn operation_uses_image_processing_lock(operation: Operation) -> bool {
    matches!(
        operation,
        Operation::MapDepthToPointCloud
            | Operation::MapDepthToPointCloudRound
            | Operation::ImageConvert
            | Operation::DepthMosaic
            | Operation::SaveImage
            | Operation::DisplayImage
    )
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

#[cfg(test)]
mod tests {
    use std::sync::{Mutex, TryLockError};

    use super::image_processing_guard;
    use crate::error::Operation;

    // 验证六个 process-wide 图像接口共用串行锁，设备接口不受影响。
    #[test]
    fn image_processing_operations_share_one_lock() {
        let lock = Mutex::new(());
        for operation in [
            Operation::MapDepthToPointCloud,
            Operation::MapDepthToPointCloudRound,
            Operation::ImageConvert,
            Operation::DepthMosaic,
            Operation::SaveImage,
            Operation::DisplayImage,
        ] {
            let guard = image_processing_guard(&lock, operation);
            assert!(guard.is_some());
            assert!(matches!(lock.try_lock(), Err(TryLockError::WouldBlock)));
            drop(guard);
            assert!(lock.try_lock().is_ok());
        }

        assert!(image_processing_guard(&lock, Operation::GetImage).is_none());
        assert!(lock.try_lock().is_ok());
    }
}
