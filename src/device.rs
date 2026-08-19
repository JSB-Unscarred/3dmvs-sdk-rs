use crate::{SdkText, SerialNumber};
use std::fmt;
use std::net::Ipv4Addr;

/// The device's reported IP configuration mode, preserving unknown SDK bits.
#[repr(transparent)]
#[derive(Clone, Copy, Default, Eq, Hash, PartialEq)]
pub struct IpConfigurationMode(u32);

impl IpConfigurationMode {
    pub const STATIC: Self = Self(0x0000_0001);
    pub const DHCP: Self = Self(0x0000_0002);
    pub const LINK_LOCAL: Self = Self(0x0000_0004);
    pub const UNDEFINED: Self = Self(0xFFFF_FFFF);

    #[must_use]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw as u32)
    }

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0 as i32
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0x0000_0001 => Some("static"),
            0x0000_0002 => Some("DHCP"),
            0x0000_0004 => Some("link-local"),
            0xFFFF_FFFF => Some("undefined"),
            _ => None,
        }
    }
}

impl fmt::Debug for IpConfigurationMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "IpConfigurationMode({name}, 0x{:08X})", self.0),
            None => write!(formatter, "IpConfigurationMode(0x{:08X})", self.0),
        }
    }
}

impl fmt::Display for IpConfigurationMode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => formatter.write_str(name),
            None => write!(formatter, "unknown mode 0x{:08X}", self.0),
        }
    }
}

/// An owned device descriptor converted from the SDK's fixed C structure.
///
/// All strings have been copied out of the SDK and no pointer or native handle
/// is retained. The snapshot owns no session lease and is `Send + Sync`.
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
