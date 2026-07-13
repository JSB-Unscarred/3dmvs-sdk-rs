use std::net::Ipv4Addr;

use crate::camera::CameraState;
use crate::driver::DriverError;
use crate::error::{ContractViolation, Error};

use super::mock_driver::{MockDriver, active_runtime, mock_handle};

#[test]
fn start_and_stop_update_state_only_after_success() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert_eq!(camera.state(), CameraState::Open);
    let measurement = camera.start().unwrap();
    assert_eq!(measurement.state(), CameraState::Measuring);
    measurement.stop().unwrap();
    assert_eq!(camera.state(), CameraState::Open);
    camera.close().unwrap();
    assert_eq!(
        mock.logs(),
        vec![
            "version",
            "initialize",
            "open_by_ip",
            "start",
            "stop",
            "close"
        ]
    );
}

#[test]
fn failed_transition_faults_the_camera() {
    let mock = MockDriver::new();
    mock.push_start(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    assert!(matches!(camera.start(), Err(Error::Sdk { .. })));
    assert_eq!(camera.state(), CameraState::Faulted);
    assert!(matches!(
        camera.clear_buffer(),
        Err(Error::InvalidState { .. })
    ));
    camera.close().unwrap();
}

#[test]
fn successful_open_with_a_null_handle_is_a_contract_violation() {
    let mock = MockDriver::new();
    mock.configure_open_by_ip(None, Ok(()));
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.open_by_ip(Ipv4Addr::LOCALHOST),
        Err(Error::ContractViolation {
            kind: ContractViolation::NullHandleOnSuccess,
            ..
        })
    ));
}

#[test]
fn handle_returned_on_open_failure_is_never_treated_as_valid() {
    let mock = MockDriver::new();
    mock.configure_open_by_ip(
        Some(mock_handle(2)),
        Err(DriverError::Status(0x8006_0005_u32 as i32)),
    );
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.open_by_ip(Ipv4Addr::LOCALHOST),
        Err(Error::OpenFailedWithHandle { .. })
    ));
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
    assert_eq!(mock.logs(), ["version", "initialize", "open_by_ip"]);
}
