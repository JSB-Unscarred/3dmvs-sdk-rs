use std::net::Ipv4Addr;

use mv3d_lp::{Camera, DeviceInfo, IpConfiguration, Result, Sdk, SdkText};

#[test]
fn public_lifecycle_and_control_methods_have_safe_signatures() {
    let _: fn() -> Result<Sdk> = Sdk::initialize;
    let _: for<'sdk> fn(&'sdk Sdk, Ipv4Addr) -> Result<Camera<'sdk>> = Sdk::open_by_ip;
    let _: for<'sdk> fn(&'sdk Sdk, &mv3d_lp::SerialNumber) -> Result<Camera<'sdk>> =
        Sdk::open_by_serial;
    let _: fn(Sdk) -> Result<()> = Sdk::shutdown;
    let _ = Camera::start;
    let _ = Camera::stop;
}

#[test]
fn owned_output_types_can_move_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<DeviceInfo>();
    assert_send_sync::<SdkText>();
    assert_send_sync::<IpConfiguration>();
}
