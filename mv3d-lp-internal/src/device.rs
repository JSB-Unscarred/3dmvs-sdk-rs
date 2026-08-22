use std::net::Ipv4Addr;

use crate::bindings;
use crate::bits::bit_newtype;
use crate::text::{SdkText, SerialNumber};

bit_newtype! {
    /// The device's reported IP configuration mode, preserving unknown SDK bits.
    pub struct IpConfigurationMode;
    STATIC = bindings::IpCfgMode_Static as u32 => "static",
    DHCP = bindings::IpCfgMode_DHCP as u32 => "DHCP",
    LINK_LOCAL = bindings::IpCfgMode_LLA as u32 => "link-local",
    // 头文件未定义 undefined 项；沿用 SDK 对未知枚举的全 1 表示。
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
    /// 模式判别值只经由 `IpConfigurationMode` 一条映射，地址字段仅 Static 需要填写。
    fn from(value: &IpConfiguration) -> Self {
        let mut raw = Self {
            mode: value.mode().raw(),
            address: [0; 16],
            subnet_mask: [0; 16],
            gateway: [0; 16],
        };
        if let IpConfiguration::Static {
            ip,
            subnet_mask,
            gateway,
        } = value
        {
            write_ipv4(&mut raw.address, *ip);
            write_ipv4(&mut raw.subnet_mask, *subnet_mask);
            write_ipv4(&mut raw.gateway, *gateway);
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

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::{IpConfigRaw, IpConfiguration};
    use crate::bindings;

    // 验证三种配置写入的 mode 与厂商头文件一致，且只有 Static 填写地址字段。
    #[test]
    fn ip_config_mode_matches_the_vendor_values() {
        let address = Ipv4Addr::new(192, 168, 1, 2);
        assert_eq!(
            IpConfigRaw::from(&IpConfiguration::Dhcp).mode,
            bindings::IpCfgMode_DHCP
        );
        assert_eq!(
            IpConfigRaw::from(&IpConfiguration::LinkLocal).mode,
            bindings::IpCfgMode_LLA
        );
        assert_eq!(IpConfigRaw::from(&IpConfiguration::Dhcp).address, [0; 16]);

        let configured =
            IpConfigRaw::from(&IpConfiguration::static_address(address, address, address));
        assert_eq!(configured.mode, bindings::IpCfgMode_Static);
        assert!(configured.address.starts_with(b"192.168.1.2\0"));
    }
}
