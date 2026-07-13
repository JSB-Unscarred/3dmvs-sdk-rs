// Expected rustc error: E0515 (the camera cannot outlive its SDK).
use std::net::Ipv4Addr;

use mv3d_lp::{Camera, Sdk};

fn camera_without_sdk() -> Camera<'static> {
    let sdk = Sdk::initialize().unwrap();
    sdk.open_by_ip(Ipv4Addr::LOCALHOST).unwrap()
}

fn main() {}
