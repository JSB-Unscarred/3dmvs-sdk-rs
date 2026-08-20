use std::net::Ipv4Addr;

use crate::bits::bit_newtype;
use crate::text::{SdkText, SerialNumber};

bit_newtype! {
    /// The device's reported IP configuration mode, preserving unknown SDK bits.
    pub struct IpConfigurationMode;
    STATIC = 0x0000_0001 => "static",
    DHCP = 0x0000_0002 => "DHCP",
    LINK_LOCAL = 0x0000_0004 => "link-local",
    UNDEFINED = 0xFFFF_FFFF => "undefined",
}

impl Default for IpConfigurationMode {
    fn default() -> Self {
        Self::UNDEFINED
    }
}

/// An owned device descriptor converted from the SDK's fixed C structure.
///
/// IP fields that are empty or not dotted-decimal IPv4 become `None` and do not fail enumeration.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct DeviceInfo {
    pub manufacturer_name: SdkText,
    pub model_name: SdkText,
    pub device_version: SdkText,
    pub manufacturer_specific_info: SdkText,
    pub serial_number: SerialNumber,
    pub user_defined_name: SdkText,
    pub mac_address: [u8; 8],
    pub ip_configuration_mode: IpConfigurationMode,
    pub current_ip: Option<Ipv4Addr>,
    pub current_subnet_mask: Option<Ipv4Addr>,
    pub default_gateway: Option<Ipv4Addr>,
    pub network_interface_ip: Option<Ipv4Addr>,
    pub device_type_info: u32,
}

/// A validated IP configuration request for `MV3D_LP_SetIpConfig`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum IpConfiguration {
    Static {
        ip: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        gateway: Ipv4Addr,
    },
    Dhcp,
    LinkLocal,
}

impl IpConfiguration {
    #[must_use]
    pub const fn static_address(ip: Ipv4Addr, subnet_mask: Ipv4Addr, gateway: Ipv4Addr) -> Self {
        Self::Static {
            ip,
            subnet_mask,
            gateway,
        }
    }

    #[must_use]
    pub const fn mode(self) -> IpConfigurationMode {
        match self {
            Self::Static { .. } => IpConfigurationMode::STATIC,
            Self::Dhcp => IpConfigurationMode::DHCP,
            Self::LinkLocal => IpConfigurationMode::LINK_LOCAL,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IpConfigRaw {
    pub(crate) mode: i32,
    pub(crate) address: [u8; 16],
    pub(crate) subnet_mask: [u8; 16],
    pub(crate) gateway: [u8; 16],
}

impl From<&IpConfiguration> for IpConfigRaw {
    fn from(value: &IpConfiguration) -> Self {
        let mut raw = Self {
            mode: 0,
            address: [0; 16],
            subnet_mask: [0; 16],
            gateway: [0; 16],
        };
        match value {
            IpConfiguration::Static {
                ip,
                subnet_mask,
                gateway,
            } => {
                raw.mode = 1;
                write_ipv4(&mut raw.address, *ip);
                write_ipv4(&mut raw.subnet_mask, *subnet_mask);
                write_ipv4(&mut raw.gateway, *gateway);
            }
            IpConfiguration::Dhcp => raw.mode = 2,
            IpConfiguration::LinkLocal => raw.mode = 4,
        }
        raw
    }
}

fn write_ipv4(destination: &mut [u8; 16], address: Ipv4Addr) {
    let text = address.to_string();
    destination[..text.len()].copy_from_slice(text.as_bytes());
}

pub(crate) fn parse_optional_ipv4(bytes: &[u8]) -> Option<Ipv4Addr> {
    if bytes.is_empty() {
        return None;
    }
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse().ok())
}
