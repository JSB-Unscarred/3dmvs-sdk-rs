use std::net::Ipv4Addr;

use crate::driver::DriverError;
use crate::error::Error;

use super::mock_driver::{MockDriver, active_runtime};

#[test]
fn drop_stops_before_closing_a_measuring_camera() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    {
        let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
        camera.start().unwrap();
    }

    let log = mock.logs();
    let cleanup = &log[log.len() - 2..];
    assert_eq!(cleanup, ["stop", "close"]);
}

#[test]
fn close_is_attempted_even_when_cleanup_stop_fails() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    mock.push_close(Err(DriverError::Status(0x8006_0000_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    camera.start().unwrap();

    let error = camera.close().unwrap_err();
    assert!(error.stop.is_some());
    assert!(error.close.is_some());
    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
    assert!(matches!(
        runtime.device_count_hint(),
        Err(Error::RuntimeTerminal)
    ));
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
    assert!(!mock.logs().contains(&"finalize"));
}

#[test]
fn explicit_close_is_not_repeated_by_drop() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    camera.close().unwrap();
    runtime.shutdown().unwrap();

    assert_eq!(
        mock.logs()
            .iter()
            .filter(|entry| **entry == "close")
            .count(),
        1
    );
    assert_eq!(
        mock.logs()
            .iter()
            .filter(|entry| **entry == "finalize")
            .count(),
        1
    );
}

#[test]
fn forgotten_camera_prevents_finalize() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    std::mem::forget(camera);

    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 1,
            teardown_uncertain: false,
        })
    ));
    assert!(!mock.logs().contains(&"finalize"));
}
