//! 枚举得到的设备信息与 IP 配置请求。

use std::ffi::{CStr, c_char};
use std::fmt;
use std::net::Ipv4Addr;

use crate::{IpConfigMode, fixed_cstr, sys, write_ipv4};

/// 枚举得到的设备信息，`MV3D_LP_DEVICE_INFO` 的副本。
///
/// 用 [`Sdk::open_by_serial`](crate::Sdk::open_by_serial) 或 [`Sdk::open_by_ip`](crate::Sdk::open_by_ip)
/// 打开对应的设备。
#[derive(Clone)]
pub struct DeviceInfo {
    raw: sys::MV3D_LP_DEVICE_INFO,
}

impl DeviceInfo {
    /// 复制 SDK 设备记录。
    pub(crate) const fn from_raw(raw: &sys::MV3D_LP_DEVICE_INFO) -> Self {
        Self { raw: *raw }
    }

    /// 厂商定义的设备类型信息。
    pub const fn device_type_info(&self) -> u32 {
        self.raw.nDevTypeInfo
    }

    /// MAC 地址，取 8 字节字段 `chMacAddress` 的前 6 字节。
    pub const fn mac_address(&self) -> [u8; 6] {
        let [byte0, byte1, byte2, byte3, byte4, byte5, _, _] = self.raw.chMacAddress;
        [byte0, byte1, byte2, byte3, byte4, byte5]
    }

    /// 制造商名称。
    pub fn manufacturer_name(&self) -> &CStr {
        fixed_cstr(&self.raw.chManufacturerName)
    }

    /// 型号名称。
    pub fn model_name(&self) -> &CStr {
        fixed_cstr(&self.raw.chModelName)
    }

    /// 序列号，可直接传给 [`Sdk::open_by_serial`](crate::Sdk::open_by_serial)。
    pub fn serial_number(&self) -> &CStr {
        fixed_cstr(&self.raw.chSerialNumber)
    }

    /// 设备版本。
    pub fn device_version(&self) -> &CStr {
        fixed_cstr(&self.raw.chDeviceVersion)
    }

    /// 用户自定义名称。
    ///
    /// 字节按设备写入时的编码保存，厂商示例按系统 ANSI 代码页（中文 Windows 上为 GBK）解码。
    pub fn user_defined_name(&self) -> &CStr {
        fixed_cstr(&self.raw.chUserDefinedName)
    }

    /// 当前的 IP 配置方式。
    pub const fn ip_config_mode(&self) -> IpConfigMode {
        IpConfigMode::from_raw(self.raw.enIPCfgMode)
    }

    /// 当前 IP；字段为空或不是点分十进制时为 `None`。
    pub fn current_ip(&self) -> Option<Ipv4Addr> {
        parse_ipv4(&self.raw.chCurrentIp)
    }

    /// 子网掩码。
    pub fn subnet_mask(&self) -> Option<Ipv4Addr> {
        parse_ipv4(&self.raw.chCurrentSubNetMask)
    }

    /// 默认网关。
    pub fn default_gateway(&self) -> Option<Ipv4Addr> {
        parse_ipv4(&self.raw.chDefultGateWay)
    }

    /// 设备所连主机网卡的 IP。
    pub fn host_ip(&self) -> Option<Ipv4Addr> {
        parse_ipv4(&self.raw.chNetExport)
    }
}

impl fmt::Debug for DeviceInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceInfo")
            .field("model_name", &self.model_name())
            .field("serial_number", &self.serial_number())
            .field("current_ip", &self.current_ip())
            .finish_non_exhaustive()
    }
}

/// [`Sdk::set_ip_config`](crate::Sdk::set_ip_config) 写入的 IP 配置。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum IpConfig {
    /// 静态 IP。
    Static {
        /// 设备 IP。
        ip: Ipv4Addr,
        /// 子网掩码。
        subnet_mask: Ipv4Addr,
        /// 默认网关。
        default_gateway: Ipv4Addr,
    },
    /// DHCP。
    Dhcp,
    /// 链路本地地址（LLA）。
    LinkLocal,
}

impl IpConfig {
    /// 转换为 SDK 结构体；只有静态 IP 填写地址字段。
    pub(crate) fn to_raw(self) -> sys::MV3D_LP_IP_CONFIG {
        let mut raw = sys::MV3D_LP_IP_CONFIG::default();
        raw.enIPCfgMode = match self {
            Self::Static {
                ip,
                subnet_mask,
                default_gateway,
            } => {
                write_ipv4(&mut raw.chDestIp, ip);
                write_ipv4(&mut raw.chDestNetMask, subnet_mask);
                write_ipv4(&mut raw.chDestGateWay, default_gateway);
                sys::IpCfgMode_Static
            }
            Self::Dhcp => sys::IpCfgMode_DHCP,
            Self::LinkLocal => sys::IpCfgMode_LLA,
        };
        raw
    }
}

/// 解析 SDK 字段中的点分十进制 IPv4 地址；字段为空或格式不对时为 `None`。
fn parse_ipv4(chars: &[c_char]) -> Option<Ipv4Addr> {
    fixed_cstr(chars).to_str().ok()?.parse().ok()
}
