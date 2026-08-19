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
pub(crate) struct DeviceListAttempt {
    pub(crate) records: Vec<DeviceRecord>,
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

fn write_ipv4(destination: &mut [u8; 16], address: Ipv4Addr) {
    let text = address.to_string();
    destination[..text.len()].copy_from_slice(text.as_bytes());
}
