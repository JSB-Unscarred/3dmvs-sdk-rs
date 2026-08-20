#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::net::Ipv4Addr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::cstr::c_string;
use crate::device::{DeviceInfo, IpConfigRaw, IpConfiguration};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRange;
use crate::driver::{DriverError, DriverResult};
use crate::error::{Error, Operation, SdkError, StatusCode};
use crate::ffi::NativeDriver;
use crate::frame::{Image, ImageFileFormat, ImageRef, ImageType};
use crate::opened_device::Device;
use crate::text::SerialNumber;

/// Owned native session shared by all session owners.
pub(crate) struct RuntimeCore {
    driver: NativeDriver,
    // 图像处理输出只在下一次处理调用前有效；同一 session 串行到 owned copy 完成。
    image_processing: Mutex<()>,
    // Close 失败后 native handle 状态未知；该单向 latch 禁止随后 Finalize。
    finalize_blocked: AtomicBool,
}

impl RuntimeCore {
    /// Calls one native operation through this session.
    pub(crate) fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&NativeDriver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let _image_processing = image_processing_guard(&self.image_processing, operation);
        call(&self.driver).map_err(|error| map_driver_error(operation, error))
    }

    /// Blocks Finalize after one device reports a failed native Close.
    pub(crate) fn block_finalize(&self) {
        self.finalize_blocked.store(true, Ordering::Release);
    }
}

#[derive(Clone)]
/// Control token for the process-wide native session.
pub struct Runtime {
    core: Arc<RuntimeCore>,
}

static INITIALIZE_CLAIMED: AtomicBool = AtomicBool::new(false);

impl Runtime {
    /// Reads the SDK version independently of the initialized session.
    pub fn version() -> Result<crate::text::SdkText, Error> {
        Self::version_bytes().map(crate::text::SdkText::from_sdk_bytes)
    }

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

    /// Initializes the native SDK using the process's sole attempt.
    pub fn initialize() -> Result<Self, Error> {
        #[cfg(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        ))]
        {
            claim_initialization(&INITIALIZE_CLAIMED)?;
            let driver = NativeDriver;
            driver
                .initialize()
                .map_err(|error| map_driver_error(Operation::Initialize, error))?;
            Ok(Self {
                core: Arc::new(RuntimeCore {
                    driver,
                    image_processing: Mutex::new(()),
                    finalize_blocked: AtomicBool::new(false),
                }),
            })
        }

        #[cfg(not(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )))]
        {
            Err(Error::UnsupportedPlatform)
        }
    }

    pub fn device_count(&self) -> Result<u32, Error> {
        self.call(Operation::GetDeviceNumber, |driver| driver.device_number())
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>, Error> {
        let count = self.device_count()?;
        if count == 0 {
            return Ok(Vec::new());
        }

        let capacity = usize::try_from(count).expect("u32 fits usize on supported targets");
        self.call(Operation::GetDeviceList, |driver| {
            driver.device_list(capacity)
        })
    }

    pub fn set_ip_config(
        &self,
        serial_number: &[u8],
        configuration: &IpConfiguration,
    ) -> Result<(), Error> {
        let serial = c_string("serial number", serial_number, Some(SerialNumber::MAX_LEN))?;
        let raw = IpConfigRaw::from(configuration);
        self.call(Operation::SetIpConfig, |driver| {
            driver.set_ip_config(&serial, &raw)
        })
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device, Error> {
        let address =
            std::ffi::CString::new(address.to_string()).expect("an IPv4 address contains no NUL");
        let handle = self.call(Operation::OpenDeviceByIp, |driver| {
            driver.open_by_ip(&address)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    pub fn open_by_serial(&self, serial_number: &[u8]) -> Result<Device, Error> {
        let serial = c_string("serial number", serial_number, Some(SerialNumber::MAX_LEN))?;
        let handle = self.call(Operation::OpenDeviceBySn, |driver| {
            driver.open_by_serial(&serial)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    pub fn map_depth_to_point_cloud(&self, input: ImageRef<'_>) -> Result<Image, Error> {
        self.call(Operation::MapDepthToPointCloud, |driver| {
            driver.map_depth_to_point_cloud(input)
        })
    }

    pub fn map_depth_to_point_cloud_round(&self, inputs: &[ImageRef<'_>]) -> Result<Image, Error> {
        self.call(Operation::MapDepthToPointCloudRound, |driver| {
            driver.map_depth_to_point_cloud_round(inputs)
        })
    }

    pub fn convert_image(&self, input: ImageRef<'_>, target: ImageType) -> Result<Image, Error> {
        self.call(Operation::ImageConvert, |driver| {
            driver.convert_image(input, target)
        })
    }

    pub fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> Result<Image, Error> {
        self.call(Operation::DepthMosaic, |driver| driver.mosaic_depth(inputs))
    }

    pub fn save_image(
        &self,
        input: ImageRef<'_>,
        format: ImageFileFormat,
        file_name: &[u8],
    ) -> Result<(), Error> {
        let file_name = c_string("file name", file_name, Some(u32::MAX as usize))?;
        self.call(Operation::SaveImage, |driver| {
            driver.save_image(input, format, &file_name)
        })
    }

    #[cfg(feature = "display-windows")]
    pub fn display_image(
        &self,
        input: ImageRef<'_>,
        window: NonZeroIsize,
        range: DisplayRange,
    ) -> Result<(), Error> {
        self.call(Operation::DisplayImage, |driver| {
            driver.display_image(input, window, range)
        })
    }

    /// Finalizes the one-shot native session.
    pub fn shutdown(self) -> Result<(), Error> {
        let core = Arc::try_unwrap(self.core).map_err(|_| Error::InvalidState {
            operation: Operation::Finalize,
            expected: "all devices dropped",
            actual: "session owners remain",
        })?;
        ensure_finalization_allowed(&core.finalize_blocked)?;
        core.driver
            .finalize()
            .map_err(|error| map_driver_error(Operation::Finalize, error))
    }

    fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&NativeDriver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        self.core.call(operation, call)
    }
}

fn claim_initialization(claimed: &AtomicBool) -> Result<(), Error> {
    if claimed.swap(true, Ordering::Relaxed) {
        Err(Error::InvalidState {
            operation: Operation::Initialize,
            expected: "not previously initialized in this process",
            actual: "already initialized",
        })
    } else {
        Ok(())
    }
}

fn image_processing_guard(lock: &Mutex<()>, operation: Operation) -> Option<MutexGuard<'_, ()>> {
    operation_uses_image_processing_lock(operation)
        .then(|| lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner()))
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
        DriverError::Status(status) => {
            Error::Sdk(SdkError::new(operation, StatusCode::from_raw(status)))
        }
        DriverError::InvalidInput { field, violation } => Error::InvalidInput { field, violation },
        DriverError::Contract(violation) => Error::ContractViolation {
            operation,
            violation,
        },
    }
}

fn ensure_finalization_allowed(blocked: &AtomicBool) -> Result<(), Error> {
    if blocked.load(Ordering::Acquire) {
        Err(Error::InvalidState {
            operation: Operation::Finalize,
            expected: "all device handles closed successfully",
            actual: "a device Close failed",
        })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, TryLockError};

    use super::{claim_initialization, ensure_finalization_allowed, image_processing_guard};
    use crate::error::{Error, Operation};

    // 验证 Initialize 机会一旦消费就不再重试。
    #[test]
    fn initialization_is_claimed_once() {
        let claimed = AtomicBool::new(false);
        assert!(claim_initialization(&claimed).is_ok());
        assert!(matches!(
            claim_initialization(&claimed),
            Err(Error::InvalidState {
                operation: Operation::Initialize,
                ..
            })
        ));
    }

    // 验证任一 Close 失败后 Finalize barrier 只允许从可用变为永久禁止。
    #[test]
    fn finalize_barrier_is_one_way() {
        let blocked = AtomicBool::new(false);
        assert!(ensure_finalization_allowed(&blocked).is_ok());

        blocked.store(true, Ordering::Release);
        for _ in 0..2 {
            assert!(matches!(
                ensure_finalization_allowed(&blocked),
                Err(Error::InvalidState {
                    operation: Operation::Finalize,
                    ..
                })
            ));
        }
    }

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
