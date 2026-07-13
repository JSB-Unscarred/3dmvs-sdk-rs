// Expected rustc error: E0505 (the camera borrows the SDK).
use std::net::Ipv4Addr;

use mv3d_lp::Sdk;

fn shutdown_with_open_camera(sdk: Sdk) {
    let camera = sdk.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    sdk.shutdown().unwrap();
    drop(camera);
}

fn main() {}
