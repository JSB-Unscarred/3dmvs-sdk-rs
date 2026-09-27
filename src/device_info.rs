//! 枚举得到的设备信息与 IP 配置请求。

use std::ffi::CStr;
use std::fmt;
use std::net::Ipv4Addr;

use crate::{IpConfigMode, fixed_cstr, sys};

/// `MV3D_LP_DEVICE_INFO` 的拥有副本。
///
/// 本值不持有 SDK 会话；打开设备见 [`Sdk::open_by_serial`](crate::Sdk::open_by_serial)。
#[derive(Clone)]
pub struct DeviceInfo {
    raw: sys::MV3D_LP_DEVICE_INFO,
}

impl DeviceInfo {
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

fn parse_ipv4(chars: &[std::os::raw::c_char]) -> Option<Ipv4Addr> {
    fixed_cstr(chars).to_str().ok()?.parse().ok()
}

/// 点分十进制最长 15 字节，16 字节字段必定能容纳文本与结尾 NUL。
pub(crate) fn write_ipv4(field: &mut [std::os::raw::c_char; 16], ip: Ipv4Addr) {
    for (target, byte) in field.iter_mut().zip(ip.to_string().bytes()) {
        *target = byte.cast_signed();
    }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::{DeviceInfo, IpConfig};
    use crate::{IpConfigMode, fixed_cstr, sys};

    // 设备字符串截断到 NUL，空 IP 字段解析为 None，MAC 取前 6 字节，未知配置方式原样保留；
    // 只有静态配置写入地址。
    #[test]
    fn device_fields_and_ip_configuration_round_trip() {
        let mut raw = sys::MV3D_LP_DEVICE_INFO::default();
        for (target, byte) in raw.chCurrentIp.iter_mut().zip(b"192.168.1.2") {
            *target = byte.cast_signed();
        }
        raw.enIPCfgMode = sys::IpCfgMode_DHCP;
        raw.chMacAddress = [1, 2, 3, 4, 5, 6, 7, 8];
        let info = DeviceInfo::from_raw(&raw);
        assert_eq!(info.current_ip(), Some(Ipv4Addr::new(192, 168, 1, 2)));
        assert_eq!(info.subnet_mask(), None);
        assert_eq!(info.ip_config_mode(), IpConfigMode::Dhcp);
        assert_eq!(info.mac_address(), [1, 2, 3, 4, 5, 6]);
        raw.enIPCfgMode = 3;
        assert_eq!(
            DeviceInfo::from_raw(&raw).ip_config_mode(),
            IpConfigMode::Other(3)
        );

        assert_eq!(fixed_cstr(&IpConfig::Dhcp.to_raw().chDestIp), c"");
        let ip = Ipv4Addr::new(10, 0, 0, 255);
        let raw = IpConfig::Static {
            ip,
            subnet_mask: ip,
            default_gateway: ip,
        }
        .to_raw();
        assert_eq!(raw.enIPCfgMode, sys::IpCfgMode_Static);
        assert_eq!(fixed_cstr(&raw.chDestGateWay), c"10.0.0.255");
    }
}
