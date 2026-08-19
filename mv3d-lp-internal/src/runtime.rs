#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ffi::CString;
use std::net::Ipv4Addr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::device::{DeviceRecord, IpConfigRaw, IpConfiguration};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{DriverError, DriverResult};
use crate::error::{ContractViolation, Error, InputViolation, Operation, SdkError, StatusCode};
use crate::ffi::NativeDriver;
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::opened_device::Device;

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
///
/// The SDK can be initialized only once per process. Dropping a token skips Finalize and relies on
/// process exit for native resource reclamation. Consuming `shutdown` performs Finalize when this
/// is the sole session owner and every device Close succeeded.
pub struct Runtime {
    core: Arc<RuntimeCore>,
}

static INITIALIZE_CLAIMED: AtomicBool = AtomicBool::new(false);

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
                violation: ContractViolation::LengthMismatch {
                    field: "device list",
                    expected: reported,
                    actual: list.records.len(),
                },
            });
        }
        Ok(list.records)
    }

    pub fn set_ip_config(
        &self,
        serial_number: &[u8],
        configuration: &IpConfiguration,
    ) -> Result<(), Error> {
        let serial = validated_c_string("serial number", serial_number, 16)?;
        let raw = IpConfigRaw::from(configuration);
        self.call(Operation::SetIpConfig, |driver| {
            driver.set_ip_config(&serial, &raw)
        })
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device, Error> {
        let address = CString::new(address.to_string()).expect("an IPv4 address contains no NUL");
        let handle = self.call(Operation::OpenDeviceByIp, |driver| {
            driver.open_by_ip(&address)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    pub fn open_by_serial(&self, serial_number: &[u8]) -> Result<Device, Error> {
        let serial = validated_c_string("serial number", serial_number, 16)?;
        let handle = self.call(Operation::OpenDeviceBySn, |driver| {
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
        let file_name = validated_c_string("file name", file_name, u32::MAX as usize)?;
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

    /// Finalizes the one-shot native session.
    ///
    /// Every `Device` and image-processing token must be dropped first. A prior device Close error
    /// permanently blocks Finalize. Consuming `self` prevents retry or overlap with another owner.
    pub fn shutdown(self) -> Result<(), Error> {
        let core = Arc::try_unwrap(self.core).map_err(|_| Error::InvalidState {
            operation: Operation::Finalize,
            expected: "all devices and image processors dropped",
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

/// Consumes the process's sole Initialize attempt and never resets it.
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

/// Serializes process-wide image helpers through the immediate SDK-output copy.
fn image_processing_guard(lock: &Mutex<()>, operation: Operation) -> Option<MutexGuard<'_, ()>> {
    operation_uses_image_processing_lock(operation)
        // The mutex is only a call gate and protects no Rust state.
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

/// Attaches public operation context to a low-level driver error.
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

fn validated_c_string(field: &'static str, bytes: &[u8], maximum: usize) -> Result<CString, Error> {
    if bytes.is_empty() {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::Empty,
        });
    }
    if bytes.len() > maximum {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::TooLong {
                max: maximum,
                actual: bytes.len(),
            },
        });
    }
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        field,
        violation: InputViolation::InteriorNul,
    })
}

/// Rejects Finalize after Close left a native handle's lifetime unknown.
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
