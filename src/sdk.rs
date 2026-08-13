use std::net::Ipv4Addr;

use crate::{
    ContractViolation, Device, DeviceInfo, Error, ImageProcessor, IpConfiguration,
    IpConfigurationMode, Operation, Result, SdkText, SerialNumber,
};

/// A token for the process-wide 3DMVS SDK session.
///
/// [`Device`] and [`ImageProcessor`] own their session access and do not borrow this value.
/// Initialize is one-shot for the process. Dropping `Sdk` leaves Finalize to process exit;
/// consuming [`Sdk::shutdown`] runs Finalize after all other session owners are dropped. `Sdk` is
/// `Send + Sync`.
pub struct Sdk {
    inner: mv3d_lp_internal::Runtime,
}

impl Sdk {
    /// Reads the SDK version without requiring Initialize.
    pub fn version() -> Result<SdkText> {
        let bytes = mv3d_lp_internal::Runtime::version_bytes()?;
        Ok(SdkText::from_sdk_bytes(bytes))
    }

    /// Initializes the process-wide SDK once.
    ///
    /// On a supported native build, later calls, including calls after an initialization error or
    /// shutdown, return [`Error::InvalidState`].
    pub fn initialize() -> Result<Self> {
        let inner = mv3d_lp_internal::Runtime::initialize()?;
        Ok(Self { inner })
    }

    pub fn device_count_hint(&self) -> Result<u32> {
        self.inner.device_count_hint()
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        self.inner
            .devices()?
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
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device> {
        self.inner.open_by_ip(address).map(Device::from_internal)
    }

    pub fn open_by_serial(&self, serial_number: &SerialNumber) -> Result<Device> {
        self.inner
            .open_by_serial(serial_number.as_bytes())
            .map(Device::from_internal)
    }

    /// Creates an owned image-processing token for this session.
    #[must_use]
    pub fn image_processor(&self) -> ImageProcessor {
        ImageProcessor {
            inner: self.inner.clone(),
        }
    }

    /// Finalizes the one-shot session and consumes this token.
    ///
    /// Drop every [`Device`] and [`ImageProcessor`] first. Remaining owners return
    /// [`Error::InvalidState`]; Finalize is not retried.
    pub fn shutdown(self) -> Result<()> {
        self.inner.shutdown()
    }
}

fn device_from_internal(record: mv3d_lp_internal::DeviceRecord) -> Result<DeviceInfo> {
    Ok(DeviceInfo {
        manufacturer_name: SdkText::from_sdk_bytes(record.manufacturer_name),
        model_name: SdkText::from_sdk_bytes(record.model_name),
        device_version: SdkText::from_sdk_bytes(record.device_version),
        manufacturer_specific_info: SdkText::from_sdk_bytes(record.manufacturer_specific_info),
        serial_number: SerialNumber::from_sdk_bytes(record.serial_number),
        user_defined_name: SdkText::from_sdk_bytes(record.user_defined_name),
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
