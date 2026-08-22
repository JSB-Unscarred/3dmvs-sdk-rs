#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::net::Ipv4Addr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::cstr::{bounded_c_string, non_empty_c_string};
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
        call(&self.driver).map_err(|error| map_driver_error(operation, error))
    }

    /// Calls one image-processing operation while holding the session's serialization lock.
    ///
    /// 图像处理输出只在下一次处理调用前有效；六个 ImgProc 接口显式选用这条入口，锁的归属
    /// 因此留在调用点，无需另一张 Operation 侧表。
    pub(crate) fn call_image_processing<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&NativeDriver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let _guard = self
            .image_processing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        self.call(operation, call)
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

    /// Reads the raw SDK version bytes; `SdkText::into_bytes` covers the same need publicly.
    fn version_bytes() -> Result<Vec<u8>, Error> {
        #[cfg(native_sdk)]
        {
            let driver = crate::ffi::NativeDriver;
            driver
                .version()
                .map_err(|error| map_driver_error(Operation::GetVersion, error))
        }

        #[cfg(not(native_sdk))]
        Err(Error::UnsupportedPlatform)
    }

    /// Initializes the native SDK using the process's sole attempt.
    pub fn initialize() -> Result<Self, Error> {
        #[cfg(native_sdk)]
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

        #[cfg(not(native_sdk))]
        {
            Err(Error::UnsupportedPlatform)
        }
    }

    /// Reads one device-count snapshot.
    pub fn device_count(&self) -> Result<u32, Error> {
        self.call(Operation::GetDeviceNumber, |driver| driver.device_number())
    }

    /// Enumerates devices as owned snapshots.
    ///
    /// count 为 0 时不进入 native GetDeviceList；capacity 由同一次 count 决定，SDK 填入的条数
    /// 可少于 capacity，多出的槽位不会被读取。
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

    /// Writes one IP configuration.
    ///
    /// 序列号先经 `bounded_c_string` 限长并拒绝 interior NUL，再交给固定宽度的 native 字段。
    pub fn set_ip_config(
        &self,
        serial_number: &[u8],
        configuration: &IpConfiguration,
    ) -> Result<(), Error> {
        let serial = bounded_c_string("serial number", serial_number, SerialNumber::MAX_LEN)?;
        let raw = IpConfigRaw::from(configuration);
        self.call(Operation::SetIpConfig, |driver| {
            driver.set_ip_config(&serial, &raw)
        })
    }

    /// Opens one device by IPv4 address.
    ///
    /// handle 只有在 status 成功且指针非空时才存在，`Device` 因此是它唯一的 owner。
    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device, Error> {
        let address =
            std::ffi::CString::new(address.to_string()).expect("an IPv4 address contains no NUL");
        let handle = self.call(Operation::OpenDeviceByIp, |driver| {
            driver.open_by_ip(&address)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    /// Opens one device by serial number.
    ///
    /// handle 只有在 status 成功且指针非空时才存在，`Device` 因此是它唯一的 owner。
    pub fn open_by_serial(&self, serial_number: &[u8]) -> Result<Device, Error> {
        let serial = bounded_c_string("serial number", serial_number, SerialNumber::MAX_LEN)?;
        let handle = self.call(Operation::OpenDeviceBySn, |driver| {
            driver.open_by_serial(&serial)
        })?;
        Ok(Device::new(Arc::clone(&self.core), handle))
    }

    /// 走 `call_image_processing`：输出只在下一次处理调用前有效，复制完成前必须串行。
    pub fn map_depth_to_point_cloud(&self, input: ImageRef<'_>) -> Result<Image, Error> {
        self.core
            .call_image_processing(Operation::MapDepthToPointCloud, |driver| {
                driver.map_depth_to_point_cloud(input)
            })
    }

    /// 走 `call_image_processing`：输出只在下一次处理调用前有效，复制完成前必须串行。
    pub fn map_depth_to_point_cloud_round(&self, inputs: &[ImageRef<'_>]) -> Result<Image, Error> {
        self.core
            .call_image_processing(Operation::MapDepthToPointCloudRound, |driver| {
                driver.map_depth_to_point_cloud_round(inputs)
            })
    }

    /// 走 `call_image_processing`：输出只在下一次处理调用前有效，复制完成前必须串行。
    pub fn convert_image(&self, input: ImageRef<'_>, target: ImageType) -> Result<Image, Error> {
        self.core
            .call_image_processing(Operation::ImageConvert, |driver| {
                driver.convert_image(input, target)
            })
    }

    /// 走 `call_image_processing`：输出只在下一次处理调用前有效，复制完成前必须串行。
    pub fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> Result<Image, Error> {
        self.core
            .call_image_processing(Operation::DepthMosaic, |driver| driver.mosaic_depth(inputs))
    }

    /// 走 `call_image_processing`：输出只在下一次处理调用前有效，复制完成前必须串行。
    pub fn save_image(
        &self,
        input: ImageRef<'_>,
        format: ImageFileFormat,
        file_name: &[u8],
    ) -> Result<(), Error> {
        let file_name = non_empty_c_string("file name", file_name)?;
        self.core
            .call_image_processing(Operation::SaveImage, |driver| {
                driver.save_image(input, format, &file_name)
            })
    }

    #[cfg(feature = "display-windows")]
    /// 走 `call_image_processing`：输出只在下一次处理调用前有效，复制完成前必须串行。
    pub fn display_image(
        &self,
        input: ImageRef<'_>,
        window: NonZeroIsize,
        range: DisplayRange,
    ) -> Result<(), Error> {
        self.core
            .call_image_processing(Operation::DisplayImage, |driver| {
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

    use super::{claim_initialization, ensure_finalization_allowed};
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
}
