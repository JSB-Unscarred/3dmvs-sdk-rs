use std::sync::Weak;
use std::time::Duration;

use crate::driver::DriverError;
use crate::error::{ContractViolation, Error, InvalidInput, Operation};
use crate::file_transfer::{FileProgressRaw, FileTransferDirection, FileTransferStatus};
use crate::opened_device::{DeviceState, take_file_name_lifetimes_for_test};

use super::mock_driver::{FfiOp, MockDriver, active_runtime};

const STATUS_CALL_FAILED: i32 = 0x8006_0005_u32 as i32;
const STATUS_CLOSE_FAILED: i32 = 0x8006_0000_u32 as i32;

fn expect_error<T, E>(result: Result<T, E>) -> E {
    match result {
        Ok(_) => panic!("operation unexpectedly succeeded"),
        Err(error) => error,
    }
}

fn take_only_file_name_lifetime() -> Weak<()> {
    let mut lifetimes = take_file_name_lifetimes_for_test();
    assert_eq!(lifetimes.len(), 1, "expected one filename bundle");
    lifetimes.pop().unwrap()
}

fn operation_count(mock: &MockDriver, expected: FfiOp) -> usize {
    mock.operations()
        .into_iter()
        .filter(|operation| *operation == expected)
        .count()
}

#[test]
fn guard_drop_keeps_names_live_and_active_transfer_can_resume() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 4,
        total: 10,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 10,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();
    assert!(names.upgrade().is_some());
    assert_eq!(transfer.direction(), FileTransferDirection::DeviceToHost);

    let calls = mock.file_access_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].operation, "file_access_read");
    assert_eq!(calls[0].user_file_name, b"host.cfg");
    assert_eq!(calls[0].device_file_name, b"device.cfg");
    assert_ne!(calls[0].user_file_name_address, 0);
    assert_ne!(calls[0].device_file_name_address, 0);

    drop(transfer);
    assert_eq!(device.state(), DeviceState::Transferring);
    assert!(names.upgrade().is_some());

    let mut resumed = device.active_file_transfer().unwrap();
    assert_eq!(resumed.direction(), FileTransferDirection::DeviceToHost);
    assert!(matches!(
        resumed.progress().unwrap(),
        FileTransferStatus::Running(progress)
            if progress.completed == 4 && progress.total == 10
    ));
    assert!(names.upgrade().is_some());
    assert!(matches!(
        resumed.progress().unwrap(),
        FileTransferStatus::Completed(progress)
            if progress.completed == 10 && progress.total == 10
    ));
    assert!(names.upgrade().is_none());
    drop(resumed);

    assert_eq!(device.state(), DeviceState::Open);
    assert!(device.active_file_transfer().is_none());
    device.clear_buffer().unwrap();
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn wait_timeout_can_retry_after_progress_diagnostic() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 0,
        total: 0,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: -1,
        total: 10,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 10,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    assert_eq!(
        transfer
            .wait_timeout(Duration::ZERO, Duration::ZERO)
            .unwrap(),
        None
    );
    assert!(matches!(
        transfer.progress(),
        Err(Error::ContractViolation {
            kind: ContractViolation::NegativeFileProgress {
                completed: -1,
                total: 10,
            },
            ..
        })
    ));
    assert!(names.upgrade().is_some());
    assert!(matches!(
        transfer.wait_timeout(Duration::ZERO, Duration::ZERO),
        Ok(Some(progress)) if progress.completed == 10 && progress.total == 10
    ));
    assert!(names.upgrade().is_none());
    drop(transfer);

    assert_eq!(device.state(), DeviceState::Open);
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::GetFileAccessProgress), 3);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn ordinary_sdk_progress_error_is_retryable() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Err(DriverError::Status(STATUS_CALL_FAILED)));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 8,
        total: 8,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    assert!(matches!(transfer.progress(), Err(Error::Sdk { .. })));
    assert!(names.upgrade().is_some());
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Completed(progress)
            if progress.completed == 8 && progress.total == 8
    ));
    assert!(names.upgrade().is_none());
    drop(transfer);

    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::GetFileAccessProgress), 2);
    runtime.shutdown().unwrap();
}

#[test]
fn progress_exceeding_its_current_total_is_retryable() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 11,
        total: 10,
    }));
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 10,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    assert_eq!(
        transfer.progress().unwrap_err(),
        Error::ContractViolation {
            operation: Operation::GetFileAccessProgress,
            kind: ContractViolation::FileProgressExceedsTotal {
                completed: 11,
                total: 10,
            },
        }
    );
    assert!(names.upgrade().is_some());
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Completed(progress)
            if progress.completed == 10 && progress.total == 10
    ));
    assert!(names.upgrade().is_none());
    drop(transfer);

    assert_eq!(device.state(), DeviceState::Open);
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::GetFileAccessProgress), 2);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn progress_snapshots_need_not_be_monotonic_or_keep_a_fixed_total() {
    let mock = MockDriver::new();
    for progress in [
        FileProgressRaw {
            completed: 4,
            total: 10,
        },
        FileProgressRaw {
            completed: 3,
            total: 12,
        },
        FileProgressRaw {
            completed: 4,
            total: 4,
        },
    ] {
        mock.push_file_access_progress(Ok(progress));
    }
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Running(progress)
            if progress.completed == 4 && progress.total == 10
    ));
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Running(progress)
            if progress.completed == 3 && progress.total == 12
    ));
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Completed(progress)
            if progress.completed == 4 && progress.total == 4
    ));
    assert!(names.upgrade().is_none());
    drop(transfer);

    assert_eq!(device.state(), DeviceState::Open);
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::GetFileAccessProgress), 3);
    runtime.shutdown().unwrap();
}

#[test]
fn filename_bundles_are_released_on_each_completion_and_never_accumulate() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 1,
        total: 1,
    }));
    let mut first = device
        .download_file(b"device-a.cfg", b"host-a.cfg")
        .unwrap();
    let first_names = take_only_file_name_lifetime();
    assert!(first_names.upgrade().is_some());
    assert!(matches!(
        first.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
    assert!(first_names.upgrade().is_none());
    drop(first);
    assert_eq!(device.state(), DeviceState::Open);

    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 2,
        total: 2,
    }));
    let mut second = device.upload_file(b"host-b.cfg", b"device-b.cfg").unwrap();
    let second_names = take_only_file_name_lifetime();
    assert_eq!(second.direction(), FileTransferDirection::HostToDevice);
    assert!(first_names.upgrade().is_none());
    assert!(second_names.upgrade().is_some());
    assert!(matches!(
        second.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
    assert!(first_names.upgrade().is_none());
    assert!(second_names.upgrade().is_none());
    drop(second);

    assert!(device.retained_file_name_addresses_for_test().is_empty());
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::FileAccessRead), 1);
    assert_eq!(operation_count(&mock, FfiOp::FileAccessWrite), 1);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    runtime.shutdown().unwrap();
}

#[test]
fn rejection_before_driver_entry_keeps_device_open_and_allocates_no_names() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let error = expect_error(device.download_file(b"", b"host-path.cfg"));
    assert!(matches!(
        error,
        Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        }
    ));
    assert_eq!(device.state(), DeviceState::Open);
    assert!(take_file_name_lifetimes_for_test().is_empty());

    let error = expect_error(device.upload_file(b"bad\0host.cfg", b"device.cfg"));
    assert!(matches!(
        error,
        Error::InvalidInput {
            kind: InvalidInput::InteriorNul,
            ..
        }
    ));
    assert_eq!(device.state(), DeviceState::Open);
    assert!(take_file_name_lifetimes_for_test().is_empty());
    assert!(mock.file_access_calls().is_empty());

    device.clear_buffer().unwrap();
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn failure_after_driver_entry_faults_device_and_retains_names_until_close() {
    let mock = MockDriver::new();
    mock.push_file_access_read(Err(DriverError::Status(STATUS_CALL_FAILED)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let error = expect_error(device.download_file(b"device-name.cfg", b"host-path.cfg"));
    let names = take_only_file_name_lifetime();
    assert!(matches!(error, Error::Sdk { .. }));
    assert_eq!(device.state(), DeviceState::Faulted);
    assert!(device.active_file_transfer().is_none());
    assert!(names.upgrade().is_some());
    assert_eq!(device.retained_file_name_addresses_for_test().len(), 1);

    device.close().unwrap();
    assert!(names.upgrade().is_none());
    let operations = mock.operations();
    assert_eq!(
        &operations[operations.len() - 2..],
        [FfiOp::FileAccessRead, FfiOp::CloseDevice]
    );
    assert_eq!(operation_count(&mock, FfiOp::FileAccessRead), 1);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn failed_start_releases_names_even_when_device_close_fails() {
    const HANDLE: usize = 0x3456_789A;

    let mock = MockDriver::new();
    mock.configure_open_by_ip(Some(HANDLE), Ok(()));
    mock.push_file_access_write(Err(DriverError::Status(STATUS_CALL_FAILED)));
    mock.push_close(Err(DriverError::Status(STATUS_CLOSE_FAILED)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let error = expect_error(device.upload_file(b"host-path.cfg", b"device-name.cfg"));
    let names = take_only_file_name_lifetime();
    assert!(matches!(error, Error::Sdk { .. }));
    assert_eq!(device.state(), DeviceState::Faulted);
    assert!(names.upgrade().is_some());

    let cleanup = device.close().unwrap_err();
    assert!(cleanup.stop.is_none());
    assert!(cleanup.close.is_some());
    assert!(names.upgrade().is_none());
    assert_eq!(mock.closed_handles(), [HANDLE]);
    assert_eq!(operation_count(&mock, FfiOp::FileAccessWrite), 1);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
}

#[test]
fn closing_an_active_transfer_releases_names_on_success() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();
    assert!(names.upgrade().is_some());
    drop(transfer);

    device.close().unwrap();
    assert!(names.upgrade().is_none());
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn closing_an_active_transfer_releases_names_on_failure() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();
    assert!(names.upgrade().is_some());
    drop(transfer);

    mock.push_close(Err(DriverError::Status(STATUS_CLOSE_FAILED)));
    let cleanup = device.close().unwrap_err();
    assert!(cleanup.stop.is_none());
    assert!(cleanup.close.is_some());
    assert!(names.upgrade().is_none());
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
}

#[test]
fn teardown_uncertainty_does_not_block_existing_transfer_progress() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 1,
        total: 2,
    }));
    let (runtime, _) = active_runtime(&mock);
    let first = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut second = runtime.open_by_serial(b"SECOND").unwrap();
    let mut transfer = second.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    mock.push_close(Err(DriverError::Status(STATUS_CLOSE_FAILED)));
    assert!(first.close().is_err());
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Running(progress)
            if progress.completed == 1 && progress.total == 2
    ));
    assert_eq!(operation_count(&mock, FfiOp::GetFileAccessProgress), 1);
    drop(transfer);

    second.close().unwrap();
    assert!(names.upgrade().is_none());
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 2);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
}

#[test]
fn borrowed_transfer_can_move_to_a_scoped_thread_then_device_is_reused() {
    let mock = MockDriver::new();
    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 1,
        total: 1,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    std::thread::scope(|scope| {
        scope
            .spawn(move || {
                let mut transfer = transfer;
                assert!(matches!(
                    transfer.progress().unwrap(),
                    FileTransferStatus::Completed(_)
                ));
            })
            .join()
            .unwrap();
    });

    assert!(names.upgrade().is_none());
    assert_eq!(device.state(), DeviceState::Open);
    device.clear_buffer().unwrap();
    device.close().unwrap();
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    runtime.shutdown().unwrap();
}
