// Expected rustc error: E0515 (the device cannot outlive its SDK).
use std::net::Ipv4Addr;

use mv3d_lp::{Device, Sdk};

fn device_without_sdk() -> Device<'static> {
    let sdk = Sdk::initialize().unwrap();
    sdk.open_by_ip(Ipv4Addr::LOCALHOST).unwrap()
}

fn main() {}
