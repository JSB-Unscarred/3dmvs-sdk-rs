use std::marker::PhantomData;
use std::net::Ipv4Addr;
use std::rc::Rc;

use crate::{
    Camera, ContractViolation, DeviceInfo, Error, ImageProcessor, IpConfiguration,
    IpConfigurationMode, Operation, Result, SdkText, SdkVersion, SerialNumber,
};

/// The process-wide 3DMVS SDK session.
///
/// A process can make one initialization attempt. Dropping or explicitly
/// shutting down this value finalizes the SDK and leaves the process in a
/// terminal state. `Sdk` is intentionally neither `Send` nor `Sync`.
pub struct Sdk {
    inner: mv3d_lp_internal::Runtime,
    version: SdkVersion,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Sdk {
    pub fn initialize() -> Result<Self> {
        let inner = mv3d_lp_internal::Runtime::initialize().map_err(Error::from)?;
        let version = std::str::from_utf8(inner.version_bytes())
            .ok()
            .and_then(|text| text.parse().ok())
            .ok_or(Error::ContractViolation {
                operation: Operation::GetVersion,
                violation: ContractViolation::InvalidValue {
                    field: "SDK version",
                },
            })?;
        Ok(Self {
            inner,
            version,
            _not_send_or_sync: PhantomData,
        })
    }

    #[must_use]
    pub const fn version(&self) -> SdkVersion {
        self.version
    }

    pub fn device_count_hint(&self) -> Result<u32> {
        self.inner.device_count_hint().map_err(Error::from)
    }

    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        self.inner
            .devices()
            .map_err(Error::from)?
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
            .map_err(Error::from)
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Camera<'_>> {
        self.inner
            .open_by_ip(address)
            .map(Camera::from_internal)
            .map_err(Error::from)
    }

    pub fn open_by_serial(&self, serial_number: &SerialNumber) -> Result<Camera<'_>> {
        self.inner
            .open_by_serial(serial_number.as_bytes())
            .map(Camera::from_internal)
            .map_err(Error::from)
    }

    #[must_use]
    pub fn image_processor(&self) -> ImageProcessor<'_> {
        ImageProcessor {
            inner: &self.inner,
            _not_send_or_sync: PhantomData,
        }
    }

    pub fn shutdown(self) -> Result<()> {
        self.inner.shutdown().map_err(Error::from)
    }
}

fn device_from_internal(record: mv3d_lp_internal::DeviceRecord) -> Result<DeviceInfo> {
    Ok(DeviceInfo {
        manufacturer_name: SdkText::new(record.manufacturer_name)?,
        model_name: SdkText::new(record.model_name)?,
        device_version: SdkText::new(record.device_version)?,
        manufacturer_specific_info: SdkText::new(record.manufacturer_specific_info)?,
        serial_number: SerialNumber::new(record.serial_number)?,
        user_defined_name: SdkText::new(record.user_defined_name)?,
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
