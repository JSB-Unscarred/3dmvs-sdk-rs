use std::net::Ipv4Addr;

use crate::{
    Device, DeviceInfo, Image, ImageFileFormat, ImageRef, ImageType, IpConfiguration, Result,
    SerialNumber,
};

/// A token for the process-wide 3DMVS SDK session.
///
/// [`Device`] owns its session access and does not borrow this value. Initialize is one-shot for
/// the process. Dropping `Sdk` skips Finalize. Consuming [`Sdk::shutdown`] runs Finalize after all
/// other session owners are dropped and every device Close succeeded. `Sdk` is `Send + Sync`.
/// Image-processing helpers are methods on this token and are serialized until their output is copied.
pub struct Sdk {
    pub(crate) inner: mv3d_lp_internal::Runtime,
}

impl Sdk {
    /// Reads the SDK version without requiring Initialize.
    ///
    /// # Errors
    ///
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn version() -> Result<crate::SdkText> {
        mv3d_lp_internal::Runtime::version()
    }

    /// Initializes the process-wide SDK once.
    ///
    /// # Errors
    ///
    /// 本进程已经初始化过（含此前初始化失败或已 shutdown）时返回 [`Error::InvalidState`](crate::Error::InvalidState)。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    pub fn initialize() -> Result<Self> {
        let inner = mv3d_lp_internal::Runtime::initialize()?;
        Ok(Self { inner })
    }

    /// Returns the current device-count snapshot. The value may change before [`Self::devices`].
    ///
    /// # Errors
    ///
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    pub fn device_count(&self) -> Result<u32> {
        self.inner.device_count()
    }

    /// Enumerates the devices visible to this session as owned snapshots.
    ///
    /// 先取一次数量再取列表；两段之间设备可能增减，返回条数可少于最新计数。
    ///
    /// # Errors
    ///
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        self.inner.devices()
    }

    /// Writes the IP configuration mode, and for a static mode the address triple, by serial number.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    pub fn set_ip_config(
        &self,
        serial_number: &SerialNumber,
        configuration: IpConfiguration,
    ) -> Result<()> {
        self.inner
            .set_ip_config(serial_number.as_bytes(), &configuration)
    }

    /// Opens one device by IPv4 address. A `Device` is produced only after a non-null handle.
    ///
    /// # Errors
    ///
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device> {
        self.inner.open_by_ip(address).map(Device::from_internal)
    }

    /// Opens one device by serial number. A `Device` is produced only after a non-null handle.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn open_by_serial(&self, serial_number: &SerialNumber) -> Result<Device> {
        self.inner
            .open_by_serial(serial_number.as_bytes())
            .map(Device::from_internal)
    }

    /// Converts one depth image to a point cloud.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn depth_to_point_cloud(&self, input: ImageRef<'_>) -> Result<Image> {
        self.inner.map_depth_to_point_cloud(input)
    }

    /// Converts multiple depth images to one round point cloud.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn depth_to_round_point_cloud(&self, inputs: &[ImageRef<'_>]) -> Result<Image> {
        self.inner.map_depth_to_point_cloud_round(inputs)
    }

    /// Converts an image to a vendor-supported target type.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn convert(&self, input: ImageRef<'_>, target: ImageType) -> Result<Image> {
        self.inner.convert_image(input, target)
    }

    /// Mosaics multiple depth images.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> Result<Image> {
        self.inner.mosaic_depth(inputs)
    }

    /// Saves an image using the vendor encoder. The name accepts `&str` or raw bytes.
    ///
    /// # Errors
    ///
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    /// SDK 返回的数据不满足其文档约定时返回 [`Error::ContractViolation`](crate::Error::ContractViolation)。
    pub fn save(
        &self,
        input: ImageRef<'_>,
        format: ImageFileFormat,
        file_name: impl AsRef<[u8]>,
    ) -> Result<()> {
        self.inner.save_image(input, format, file_name.as_ref())
    }

    /// Finalizes the one-shot session and consumes this token.
    ///
    /// # Errors
    ///
    /// 仍有其他 session owner 未 drop，或此前有设备 Close 失败时返回 [`Error::InvalidState`](crate::Error::InvalidState)。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    pub fn shutdown(self) -> Result<()> {
        self.inner.shutdown()
    }
}
