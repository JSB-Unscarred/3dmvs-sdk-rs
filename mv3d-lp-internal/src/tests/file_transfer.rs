use crate::camera::CameraState;
use crate::driver::DriverError;
use crate::error::{ContractViolation, Error, InvalidInput};
use crate::file_transfer::{FileProgressRaw, FileTransferDirection, FileTransferStatus};

use super::mock_driver::{MockDriver, active_runtime};

#[test]
fn transfer_names_survive_guard_drop_and_polling_can_resume() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 0,
        total: 0,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 8,
        total: 8,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let transfer = camera.download_file(b"device.cfg", b"host.cfg").unwrap();
    assert_eq!(transfer.direction(), FileTransferDirection::DeviceToHost);
    let calls = mock.file_access_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].operation, "file_access_read");
    assert_eq!(calls[0].user_file_name, b"host.cfg");
    assert_eq!(calls[0].device_file_name, b"device.cfg");
    assert_ne!(calls[0].user_file_name_address, 0);
    assert_ne!(calls[0].device_file_name_address, 0);
    drop(transfer);
    assert_eq!(camera.state(), CameraState::Transferring);

    let mut resumed = camera.active_file_transfer().unwrap();
    assert!(matches!(
        resumed.progress().unwrap(),
        FileTransferStatus::Running(progress) if progress.completed == 0 && progress.total == 0
    ));
    assert!(matches!(
        resumed.progress().unwrap(),
        FileTransferStatus::Completed(progress) if progress.completed == 8 && progress.total == 8
    ));
    drop(resumed);
    assert_eq!(camera.state(), CameraState::Open);
}

#[test]
fn malformed_or_regressing_progress_keeps_transfer_active() {
    let mock = MockDriver::new();
    mock.push_file_access_write(Ok(()));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 4,
        total: 10,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 3,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = camera.upload_file(b"host.cfg", b"device.cfg").unwrap();
    let calls = mock.file_access_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].operation, "file_access_write");
    assert_eq!(calls[0].user_file_name, b"host.cfg");
    assert_eq!(calls[0].device_file_name, b"device.cfg");

    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Running(_)
    ));
    assert!(matches!(
        transfer.progress(),
        Err(Error::ContractViolation {
            kind: ContractViolation::FileProgressRegressed {
                previous: 4,
                current: 3,
            },
            ..
        })
    ));
    drop(transfer);
    assert_eq!(camera.state(), CameraState::Transferring);
}

#[test]
fn changing_a_nonzero_total_cannot_end_the_transfer_early() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 4,
        total: 10,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 4,
        total: 4,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 10,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = camera.download_file(b"device.cfg", b"host.cfg").unwrap();

    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Running(_)
    ));
    assert!(matches!(
        transfer.progress(),
        Err(Error::ContractViolation {
            kind: ContractViolation::FileProgressTotalChanged {
                previous: 10,
                current: 4,
            },
            ..
        })
    ));
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
}

#[test]
fn progress_errors_never_release_names_or_unlock_camera_operations() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: -1,
        total: 10,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 11,
        total: 10,
    }));
    mock.push_file_access_progress(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 10,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let mut transfer = camera.download_file(b"device.cfg", b"host.cfg").unwrap();
    assert!(transfer.progress().is_err());
    assert!(transfer.progress().is_err());
    assert!(transfer.progress().is_err());
    drop(transfer);
    assert_eq!(camera.state(), CameraState::Transferring);
    assert!(camera.start().is_err());
    assert!(
        camera
            .download_file(b"other.cfg", b"other-host.cfg")
            .is_err()
    );

    let mut resumed = camera.active_file_transfer().unwrap();
    assert!(matches!(
        resumed.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
}

#[test]
fn closing_an_active_transfer_does_not_stop_measurement() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    drop(camera.download_file(b"device.cfg", b"host.cfg").unwrap());

    camera.close().unwrap();
    let logs = mock.logs();
    assert!(logs.contains(&"close"));
    assert!(!logs.contains(&"stop"));
}

#[test]
fn invalid_names_are_rejected_before_the_driver_call() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    assert!(matches!(
        camera.download_file(b"", b"host.cfg"),
        Err(Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        })
    ));
    assert!(matches!(
        camera.upload_file(b"bad\0name", b"device.cfg"),
        Err(Error::InvalidInput {
            kind: InvalidInput::InteriorNul,
            ..
        })
    ));
    assert!(!mock.logs().contains(&"file_access_read"));
    assert!(!mock.logs().contains(&"file_access_write"));
}

#[test]
fn failed_start_is_faulted_and_close_is_the_only_cleanup_call() {
    let mock = MockDriver::new();
    mock.push_file_access_read(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    assert!(camera.download_file(b"device.cfg", b"host.cfg").is_err());
    assert_eq!(camera.state(), CameraState::Faulted);
    camera.close().unwrap();
    let logs = mock.logs();
    assert!(logs.contains(&"close"));
    assert!(!logs.contains(&"stop"));
}
