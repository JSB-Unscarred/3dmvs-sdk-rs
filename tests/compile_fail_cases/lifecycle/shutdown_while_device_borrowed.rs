// Expected rustc error: E0505 (the device borrows the SDK).
use std::net::Ipv4Addr;

use mv3d_lp::Sdk;

fn shutdown_with_open_device(sdk: Sdk) {
    let device = sdk.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    sdk.shutdown().unwrap();
    drop(device);
}

fn main() {}
