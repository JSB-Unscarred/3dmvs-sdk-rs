use std::net::Ipv4Addr;

use crate::driver::DriverError;
use crate::error::{Error, InvalidInput, Operation};
use crate::frame::{FrameRecord, ImageTypeRecord};
use crate::opened_device::DeviceState;

use super::mock_driver::{MockDriver, active_runtime};

fn frame(data: Vec<u8>) -> FrameRecord {
    FrameRecord {
        image_type: ImageTypeRecord::from_bits(0x0108_0001),
        width: u32::try_from(data.len()).unwrap(),
        height: 1,
        data,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 7,
        device_timestamp: 11,
        valid: true,
        x_scale: 1.0,
        y_scale: 2.0,
        z_scale: 3.0,
        x_offset: 4,
        y_offset: 5,
        z_offset: 6,
    }
}

#[test]
fn measurement_routes_pull_controls_and_explicit_stop() {
    let mock = MockDriver::new();
    mock.push_get_image(Ok(frame(vec![1, 2, 3])));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let mut measurement = device.start().unwrap();
    measurement.soft_trigger().unwrap();
    measurement.clear_buffer().unwrap();
    let returned = measurement.get_image(37).unwrap();
    assert_eq!(returned.data, [1, 2, 3]);
    measurement.stop().unwrap();
    assert_eq!(device.state(), DeviceState::Open);
    device.close().unwrap();

    assert_eq!(mock.image_timeouts(), [37]);
    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "open_by_ip",
            "start",
            "soft_trigger",
            "clear_buffer",
            "get_image",
            "stop",
            "close",
        ]
    );
}

#[test]
fn no_data_is_exact_and_the_measurement_can_retry() {
    let mock = MockDriver::new();
    mock.push_get_image(Err(DriverError::Status(0x8006_0006_u32 as i32)));
    mock.push_get_image(Ok(frame(vec![9])));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let mut measurement = device.start().unwrap();

    assert!(matches!(
        measurement.get_image(5),
        Err(Error::Sdk {
            operation: Operation::GetImage,
            status,
        }) if status as u32 == 0x8006_0006
    ));
    assert_eq!(measurement.get_image(6).unwrap().data, [9]);
    measurement.stop().unwrap();
    device.close().unwrap();

    assert_eq!(mock.image_timeouts(), [5, 6]);
}

#[test]
fn disconnect_preserves_status_and_cleanup_still_runs() {
    let mock = MockDriver::new();
    mock.push_get_image(Err(DriverError::Status(0x8006_000D_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let mut measurement = device.start().unwrap();

    assert!(matches!(
        measurement.get_image(25),
        Err(Error::Sdk {
            operation: Operation::GetImage,
            status,
        }) if status as u32 == 0x8006_000D
    ));
    measurement.stop().unwrap();
    device.close().unwrap();

    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
}

#[test]
fn dropped_measurement_stops_before_device_close() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    drop(device.start().unwrap());
    assert_eq!(device.state(), DeviceState::Open);
    device.close().unwrap();

    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
}

#[test]
fn failed_explicit_stop_faults_device_and_close_retries_cleanup_stop() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let measurement = device.start().unwrap();
    assert!(matches!(measurement.stop(), Err(Error::Sdk { .. })));
    assert_eq!(device.state(), DeviceState::Faulted);
    device.close().unwrap();

    let stop_count = mock.logs().iter().filter(|entry| **entry == "stop").count();
    assert_eq!(stop_count, 2);
}

#[test]
fn failed_drop_stop_faults_device_and_close_retries_once() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    drop(device.start().unwrap());
    assert_eq!(device.state(), DeviceState::Faulted);
    device.close().unwrap();

    let log = mock.logs();
    assert_eq!(&log[log.len() - 3..], ["stop", "stop", "close"]);
}

#[test]
fn infinite_timeout_sentinel_is_rejected_before_driver() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let mut measurement = device.start().unwrap();

    assert!(matches!(
        measurement.get_image(u32::MAX),
        Err(Error::InvalidInput {
            kind: InvalidInput::TimeoutTooLong { .. },
            ..
        })
    ));
    assert!(mock.image_timeouts().is_empty());
    measurement.stop().unwrap();
    device.close().unwrap();
}

#[test]
fn trigger_and_clear_errors_do_not_end_the_session() {
    let mock = MockDriver::new();
    mock.push_soft_trigger(Err(DriverError::Status(0x8006_0007_u32 as i32)));
    mock.push_clear_buffer(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let mut measurement = device.start().unwrap();

    assert!(matches!(measurement.soft_trigger(), Err(Error::Sdk { .. })));
    assert!(matches!(measurement.clear_buffer(), Err(Error::Sdk { .. })));
    assert_eq!(measurement.state(), DeviceState::Measuring);
    measurement.stop().unwrap();
    device.close().unwrap();
}
