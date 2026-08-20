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
    pub fn version() -> Result<crate::SdkText> {
        mv3d_lp_internal::Runtime::version()
    }

    /// Initializes the process-wide SDK once.
    pub fn initialize() -> Result<Self> {
        let inner = mv3d_lp_internal::Runtime::initialize()?;
        Ok(Self { inner })
    }

    /// Returns the current device-count snapshot. The value may change before [`Self::devices`].
    pub fn device_count(&self) -> Result<u32> {
        self.inner.device_count()
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        self.inner.devices()
    }

    pub fn set_ip_config(
        &self,
        serial_number: &SerialNumber,
        configuration: IpConfiguration,
    ) -> Result<()> {
        self.inner
            .set_ip_config(serial_number.as_bytes(), &configuration)
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device> {
        self.inner.open_by_ip(address).map(Device::from_internal)
    }

    pub fn open_by_serial(&self, serial_number: &SerialNumber) -> Result<Device> {
        self.inner
            .open_by_serial(serial_number.as_bytes())
            .map(Device::from_internal)
    }

    /// Converts one depth image to a point cloud.
    pub fn depth_to_point_cloud(&self, input: ImageRef<'_>) -> Result<Image> {
        self.inner.map_depth_to_point_cloud(input)
    }

    /// Converts multiple depth images to one round point cloud.
    pub fn depth_to_round_point_cloud(&self, inputs: &[ImageRef<'_>]) -> Result<Image> {
        self.inner.map_depth_to_point_cloud_round(inputs)
    }

    /// Converts an image to a vendor-supported target type.
    pub fn convert(&self, input: ImageRef<'_>, target: ImageType) -> Result<Image> {
        self.inner.convert_image(input, target)
    }

    /// Mosaics multiple depth images.
    pub fn mosaic_depth(&self, inputs: &[ImageRef<'_>]) -> Result<Image> {
        self.inner.mosaic_depth(inputs)
    }

    /// Saves an image using the vendor encoder.
    pub fn save(
        &self,
        input: ImageRef<'_>,
        format: ImageFileFormat,
        file_name: &[u8],
    ) -> Result<()> {
        self.inner.save_image(input, format, file_name)
    }

    /// Finalizes the one-shot session and consumes this token.
    pub fn shutdown(self) -> Result<()> {
        self.inner.shutdown()
    }
}
