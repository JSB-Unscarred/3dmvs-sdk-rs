use std::net::Ipv4Addr;

use crate::{
    ContractViolation, Device, DeviceInfo, Error, ImageProcessor, IpConfiguration,
    IpConfigurationMode, Operation, Result, SdkText, SdkVersion, SerialNumber,
};

/// A token for the process-wide 3DMVS SDK session.
///
/// [`Device`] and [`ImageProcessor`] own their session access and do not borrow this value.
/// Dropping an `Sdk` leaves the session active; call [`Sdk::shutdown`] after every device closes to
/// run `Finalize` and observe its result. Successful shutdown makes native operations through
/// other tokens from that session return [`Error::RuntimeInactive`]. `Sdk` is `Send + Sync`.
pub struct Sdk {
    inner: mv3d_lp_internal::Runtime,
    version: SdkVersion,
}

impl Sdk {
    /// Initializes the process-wide SDK or joins its active session.
    pub fn initialize() -> Result<Self> {
        let inner = mv3d_lp_internal::Runtime::initialize().map_err(Error::map_internal_error)?;
        Self::from_internal(inner)
    }

    /// Initializes or reuses the process-wide SDK only when its version exactly matches the
    /// audited bindings baseline.
    pub fn initialize_strict() -> Result<Self> {
        let inner =
            mv3d_lp_internal::Runtime::initialize_strict().map_err(Error::map_internal_error)?;
        Self::from_internal(inner)
    }

    fn from_internal(inner: mv3d_lp_internal::Runtime) -> Result<Self> {
        let version = std::str::from_utf8(inner.version_bytes())
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or(Error::ContractViolation {
                operation: Operation::GetVersion,
                violation: ContractViolation::InvalidValue {
                    field: "SDK version",
                },
            })?;
        Ok(Self { inner, version })
    }

    /// Returns the cached SDK version, including after this token's session is finalized.
    #[must_use]
    pub const fn version(&self) -> SdkVersion {
        self.version
    }

    pub fn device_count_hint(&self) -> Result<u32> {
        self.inner
            .device_count_hint()
            .map_err(Error::map_internal_error)
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        self.inner
            .devices()
            .map_err(Error::map_internal_error)?
            .into_iter()
            .map(device_from_internal)
            .collect()
    }

    pub fn set_ip_config(
        &self,
        serial_number: &SerialNumber,
        configuration: IpConfiguration,
    ) -> Result<()> {
        let internal_configuration = match configuration {
            IpConfiguration::Static {
                ip,
                subnet_mask,
                gateway,
            } => mv3d_lp_internal::IpConfiguration::Static {
                address: ip,
                subnet_mask,
                gateway,
            },
            IpConfiguration::Dhcp => mv3d_lp_internal::IpConfiguration::Dhcp,
            IpConfiguration::LinkLocal => mv3d_lp_internal::IpConfiguration::LinkLocal,
        };
        self.inner
            .set_ip_config(serial_number.as_bytes(), &internal_configuration)
            .map_err(Error::map_internal_error)
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device> {
        self.inner
            .open_by_ip(address)
            .map(Device::from_internal)
            .map_err(Error::map_internal_error)
    }

    pub fn open_by_serial(&self, serial_number: &SerialNumber) -> Result<Device> {
        self.inner
            .open_by_serial(serial_number.as_bytes())
            .map(Device::from_internal)
            .map_err(Error::map_internal_error)
    }

    /// Creates an owned image-processing token for the active session.
    #[must_use]
    pub fn image_processor(&self) -> ImageProcessor {
        ImageProcessor {
            inner: self.inner.clone(),
        }
    }

    /// Finalizes the active session after every `Device` has closed.
    ///
    /// A live device returns [`Error::UnclosedDevices`] and leaves the session active. Close every
    /// device and retry. A repeated call returns `Ok(())` while no newer session is active; an old
    /// token cannot finalize a newer session.
    pub fn shutdown(&self) -> Result<()> {
        self.inner.shutdown().map_err(Error::map_internal_error)
    }
}

fn device_from_internal(record: mv3d_lp_internal::DeviceRecord) -> Result<DeviceInfo> {
    Ok(DeviceInfo {
        manufacturer_name: SdkText::try_from(record.manufacturer_name)?,
        model_name: SdkText::try_from(record.model_name)?,
        device_version: SdkText::try_from(record.device_version)?,
        manufacturer_specific_info: SdkText::try_from(record.manufacturer_specific_info)?,
        serial_number: SerialNumber::try_from(record.serial_number)?,
        user_defined_name: SdkText::try_from(record.user_defined_name)?,
        mac_address: record.mac_address,
        ip_configuration_mode: IpConfigurationMode::from_raw(record.ip_configuration_mode),
        current_ip: parse_optional_ipv4("current IP", &record.current_ip)?,
        current_subnet_mask: parse_optional_ipv4(
            "current subnet mask",
            &record.current_subnet_mask,
        )?,
        default_gateway: parse_optional_ipv4("default gateway", &record.default_gateway)?,
        network_interface_ip: parse_optional_ipv4("network interface IP", &record.interface_ip)?,
        device_type_info: record.device_type,
    })
}

fn parse_optional_ipv4(field: &'static str, bytes: &[u8]) -> Result<Option<Ipv4Addr>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let address = std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse().ok())
        .ok_or(Error::ContractViolation {
            operation: Operation::GetDeviceList,
            violation: ContractViolation::InvalidValue { field },
        })?;
    Ok(Some(address))
}
