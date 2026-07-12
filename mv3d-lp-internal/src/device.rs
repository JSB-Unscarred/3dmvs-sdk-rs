use std::net::Ipv4Addr;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceRecord {
    pub manufacturer_name: Vec<u8>,
    pub model_name: Vec<u8>,
    pub device_version: Vec<u8>,
    pub manufacturer_specific_info: Vec<u8>,
    pub serial_number: Vec<u8>,
    pub user_defined_name: Vec<u8>,
    pub mac_address: [u8; 8],
    pub ip_configuration_mode: i32,
    pub current_ip: Vec<u8>,
    pub current_subnet_mask: Vec<u8>,
    pub default_gateway: Vec<u8>,
    pub interface_ip: Vec<u8>,
    pub device_type: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IpConfiguration {
    Static {
        address: Ipv4Addr,
        subnet_mask: Ipv4Addr,
        gateway: Ipv4Addr,
    },
    Dhcp,
    LinkLocal,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceInfoRaw {
    pub(crate) manufacturer_name: [u8; 32],
    pub(crate) model_name: [u8; 32],
    pub(crate) device_version: [u8; 32],
    pub(crate) manufacturer_specific_info: [u8; 48],
    pub(crate) serial_number: [u8; 16],
    pub(crate) user_defined_name: [u8; 16],
    pub(crate) mac_address: [u8; 8],
    pub(crate) ip_configuration_mode: i32,
    pub(crate) current_ip: [u8; 16],
    pub(crate) current_subnet_mask: [u8; 16],
    pub(crate) default_gateway: [u8; 16],
    pub(crate) interface_ip: [u8; 16],
    pub(crate) device_type: u32,
}

impl Default for DeviceInfoRaw {
    fn default() -> Self {
        Self {
            manufacturer_name: [0; 32],
            model_name: [0; 32],
            device_version: [0; 32],
            manufacturer_specific_info: [0; 48],
            serial_number: [0; 16],
            user_defined_name: [0; 16],
            mac_address: [0; 8],
            ip_configuration_mode: 0,
            current_ip: [0; 16],
            current_subnet_mask: [0; 16],
            default_gateway: [0; 16],
            interface_ip: [0; 16],
            device_type: 0,
        }
    }
}

impl From<DeviceInfoRaw> for DeviceRecord {
    fn from(raw: DeviceInfoRaw) -> Self {
        Self {
            manufacturer_name: bounded_bytes(&raw.manufacturer_name),
            model_name: bounded_bytes(&raw.model_name),
            device_version: bounded_bytes(&raw.device_version),
            manufacturer_specific_info: bounded_bytes(&raw.manufacturer_specific_info),
            serial_number: bounded_bytes(&raw.serial_number),
            user_defined_name: bounded_bytes(&raw.user_defined_name),
            mac_address: raw.mac_address,
            ip_configuration_mode: raw.ip_configuration_mode,
            current_ip: bounded_bytes(&raw.current_ip),
            current_subnet_mask: bounded_bytes(&raw.current_subnet_mask),
            default_gateway: bounded_bytes(&raw.default_gateway),
            interface_ip: bounded_bytes(&raw.interface_ip),
            device_type: raw.device_type,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeviceListAttempt {
    pub(crate) records: Vec<DeviceInfoRaw>,
    pub(crate) reported: u32,
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
                address,
                subnet_mask,
                gateway,
            } => {
                raw.mode = 1;
                write_ipv4(&mut raw.address, *address);
                write_ipv4(&mut raw.subnet_mask, *subnet_mask);
                write_ipv4(&mut raw.gateway, *gateway);
            }
            IpConfiguration::Dhcp => raw.mode = 2,
            IpConfiguration::LinkLocal => raw.mode = 4,
        }
        raw
    }
}

pub(crate) fn bounded_bytes<const N: usize>(bytes: &[u8; N]) -> Vec<u8> {
    let length = bytes.iter().position(|byte| *byte == 0).unwrap_or(N);
    bytes[..length].to_vec()
}

fn write_ipv4(destination: &mut [u8; 16], address: Ipv4Addr) {
    let text = address.to_string();
    destination[..text.len()].copy_from_slice(text.as_bytes());
}
