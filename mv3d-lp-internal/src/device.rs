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
    /// 厂商名。
    pub manufacturer_name: SdkText,
    /// 型号名。
    pub model_name: SdkText,
    /// 设备固件版本。
    pub device_version: SdkText,
    /// 厂商自定义信息。
    pub manufacturer_specific_info: SdkText,
    /// 设备序列号，同时是按序列号打开设备的键。
    pub serial_number: SerialNumber,
    /// 用户自定义名称。
    pub user_defined_name: SdkText,
    /// 设备 MAC 地址，按 SDK 字段宽度保留 8 字节。
    pub mac_address: [u8; 8],
    /// 设备当前的 IP 配置方式。
    pub ip_configuration_mode: IpConfigurationMode,
    /// 设备当前 IP；字段为空或非点分十进制时为 `None`。
    pub current_ip: Option<Ipv4Addr>,
    /// 设备当前子网掩码。
    pub current_subnet_mask: Option<Ipv4Addr>,
    /// 设备默认网关。
    pub default_gateway: Option<Ipv4Addr>,
    /// 该设备所在主机网卡的 IP。
    pub network_interface_ip: Option<Ipv4Addr>,
    /// 厂商定义的设备类型位。
    pub device_type_info: u32,
}

/// A validated IP configuration request for `MV3D_LP_SetIpConfig`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum IpConfiguration {
    /// 静态 IP，三个地址全部由调用方指定。
    Static {
        /// 设备 IP。
        ip: Ipv4Addr,
        /// 子网掩码。
        subnet_mask: Ipv4Addr,
        /// 默认网关。
        gateway: Ipv4Addr,
    },
    /// 由 DHCP 分配。
    Dhcp,
    /// 使用 LLA 链路本地地址。
    LinkLocal,
}

impl IpConfiguration {
    /// 构造一个静态 IP 配置。
    #[must_use]
    pub const fn static_address(ip: Ipv4Addr, subnet_mask: Ipv4Addr, gateway: Ipv4Addr) -> Self {
        Self::Static {
            ip,
            subnet_mask,
            gateway,
        }
    }

    /// 返回该配置对应的 SDK 配置方式取值。
    #[must_use]
    pub const fn mode(self) -> IpConfigurationMode {
        match self {
            Self::Static { .. } => IpConfigurationMode::STATIC,
            Self::Dhcp => IpConfigurationMode::DHCP,
            Self::LinkLocal => IpConfigurationMode::LINK_LOCAL,
        }
    }
}

pub fn parse_optional_ipv4(bytes: &[u8]) -> Option<Ipv4Addr> {
    if bytes.is_empty() {
        return None;
    }
    std::str::from_utf8(bytes)
        .ok()
        .and_then(|text| text.parse().ok())
}
