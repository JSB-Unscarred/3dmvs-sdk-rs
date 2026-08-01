use std::fmt;
use std::sync::Weak;
use std::time::Duration;

use crate::driver::DriverError;
use crate::error::{ContractViolation, Error, InvalidInput};
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

fn file_access_addresses(mock: &MockDriver, index: usize) -> (usize, usize) {
    let calls = mock.file_access_calls();
    let call = &calls[index];
    assert_ne!(call.user_file_name_address, 0);
    assert_ne!(call.device_file_name_address, 0);
    (call.user_file_name_address, call.device_file_name_address)
}

fn assert_redacted<T: fmt::Debug + fmt::Display>(value: &T, secrets: &[&str]) {
    let debug = format!("{value:?}");
    let display = value.to_string();
    let debug_lower = debug.to_ascii_lowercase();
    let display_lower = display.to_ascii_lowercase();

    for secret in secrets {
        let secret = secret.to_ascii_lowercase();
        assert!(
            !debug_lower.contains(&secret),
            "Debug output leaked `{secret}`: {debug}"
        );
        assert!(
            !display_lower.contains(&secret),
            "Display output leaked `{secret}`: {display}"
        );
    }
}

#[test]
fn running_completed_cache_wait_and_device_recovery_form_one_state_machine() {
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
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();
    let addresses = file_access_addresses(&mock, 0);
    assert!(names.upgrade().is_some());
    assert_eq!(transfer.direction(), FileTransferDirection::DeviceToHost);
    assert_eq!(
        transfer.retained_file_name_addresses_for_test(),
        [addresses]
    );
    assert!(names.upgrade().is_some());

    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Running(progress)
            if progress.completed == 4 && progress.total == 10
    ));
    let mut transfer = match transfer.try_into_device() {
        Ok(_) => panic!("a running transfer returned its device"),
        Err(transfer) => transfer,
    };

    let final_progress = match transfer.progress().unwrap() {
        FileTransferStatus::Completed(progress) => progress,
        FileTransferStatus::Running(_) => panic!("transfer did not complete"),
    };
    assert_eq!(transfer.retained_file_name_addresses_for_test(), []);
    assert!(names.upgrade().is_none());
    let progress_calls = operation_count(&mock, FfiOp::GetFileAccessProgress);

    assert_eq!(
        transfer.progress().unwrap(),
        FileTransferStatus::Completed(final_progress)
    );
    assert_eq!(
        transfer
            .wait_timeout(Duration::from_secs(1), Duration::from_secs(1))
            .unwrap(),
        Some(final_progress)
    );
    assert_eq!(
        operation_count(&mock, FfiOp::GetFileAccessProgress),
        progress_calls,
        "completed progress must be served from the final cache"
    );

    let (device, recovered_progress) = match transfer.try_into_device() {
        Ok(recovered) => recovered,
        Err(_) => panic!("a completed transfer did not return its device"),
    };
    assert_eq!(recovered_progress, final_progress);
    assert_eq!(device.state(), DeviceState::Open);
    assert!(device.retained_file_name_addresses_for_test().is_empty());
    assert!(names.upgrade().is_none());

    device.close().unwrap();
    assert!(names.upgrade().is_none());
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
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
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
    let (device, _) = match transfer.try_into_device() {
        Ok(recovered) => recovered,
        Err(_) => panic!("a completed retry did not return its device"),
    };
    device.close().unwrap();
    assert!(names.upgrade().is_none());
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
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    assert!(matches!(transfer.progress(), Err(Error::Sdk { .. })));
    assert!(matches!(
        transfer.progress().unwrap(),
        FileTransferStatus::Completed(progress)
            if progress.completed == 8 && progress.total == 8
    ));
    let (device, _) = match transfer.try_into_device() {
        Ok(recovered) => recovered,
        Err(_) => panic!("a completed retry did not return its device"),
    };
    device.close().unwrap();
    assert!(names.upgrade().is_none());
    assert_eq!(operation_count(&mock, FfiOp::GetFileAccessProgress), 2);
    runtime.shutdown().unwrap();
}

#[test]
fn progress_contract_violations_are_retryable() {
    let cases = [
        (
            vec![],
            FileProgressRaw {
                completed: 11,
                total: 10,
            },
            ContractViolation::FileProgressExceedsTotal {
                completed: 11,
                total: 10,
            },
        ),
        (
            vec![FileProgressRaw {
                completed: 4,
                total: 10,
            }],
            FileProgressRaw {
                completed: 3,
                total: 10,
            },
            ContractViolation::FileProgressRegressed {
                previous: 4,
                current: 3,
            },
        ),
        (
            vec![FileProgressRaw {
                completed: 4,
                total: 10,
            }],
            FileProgressRaw {
                completed: 4,
                total: 4,
            },
            ContractViolation::FileProgressTotalChanged {
                previous: 10,
                current: 4,
            },
        ),
    ];

    for (prefix, invalid, expected) in cases {
        let mock = MockDriver::new();
        for sample in &prefix {
            mock.push_file_access_progress(Ok(*sample));
        }
        mock.push_file_access_progress(Ok(invalid));
        mock.push_file_access_progress(Ok(FileProgressRaw {
            completed: 10,
            total: 10,
        }));
        let (runtime, _) = active_runtime(&mock);
        let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
        let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
        let names = take_only_file_name_lifetime();

        for _ in &prefix {
            assert!(matches!(
                transfer.progress().unwrap(),
                FileTransferStatus::Running(_)
            ));
        }
        assert_eq!(
            transfer.progress().unwrap_err(),
            Error::ContractViolation {
                operation: "MV3D_LP_GetFileAccessProgress",
                kind: expected,
            }
        );
        assert!(names.upgrade().is_some());
        assert!(matches!(
            transfer.progress().unwrap(),
            FileTransferStatus::Completed(progress)
                if progress.completed == 10 && progress.total == 10
        ));
        let (device, _) = match transfer.try_into_device() {
            Ok(recovered) => recovered,
            Err(_) => panic!("a completed retry did not return its device"),
        };
        device.close().unwrap();
        assert!(names.upgrade().is_none());
        assert_eq!(
            operation_count(&mock, FfiOp::GetFileAccessProgress),
            prefix.len() + 2
        );
        assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
        assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
        runtime.shutdown().unwrap();
    }
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
    let second = runtime.open_by_serial(b"SECOND").unwrap();
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

    transfer.close().unwrap();
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
fn pre_entry_validation_can_recover_or_drop_the_device_exactly_once() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let error = expect_error(device.download_file(b"", b"secret-host-path.cfg"));
    assert!(matches!(
        error.start_error(),
        Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        }
    ));
    assert!(error.cleanup_error().is_none());
    assert_redacted(&error, &["secret-host-path.cfg"]);
    assert!(take_file_name_lifetimes_for_test().is_empty());

    let (source, mut device) = error.into_rejected_device().unwrap();
    assert!(matches!(
        source,
        Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        }
    ));
    assert_eq!(device.state(), DeviceState::Open);
    device.clear_buffer().unwrap();
    device.close().unwrap();

    let device = runtime.open_by_serial(b"SECOND").unwrap();
    let error = expect_error(device.upload_file(b"secret\0host.cfg", b"device.cfg"));
    assert!(matches!(
        error.start_error(),
        Error::InvalidInput {
            kind: InvalidInput::InteriorNul,
            ..
        }
    ));
    drop(error);

    assert!(mock.file_access_calls().is_empty());
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 2);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn teardown_uncertainty_allows_driver_entry_for_an_existing_device() {
    const FIRST_HANDLE: usize = 0x1234_5678;
    const SECOND_HANDLE: usize = 0x2345_6789;

    let mock = MockDriver::new();
    mock.configure_open_by_ip(Some(FIRST_HANDLE), Ok(()));
    mock.configure_open_by_serial(Some(SECOND_HANDLE), Ok(()));
    let (runtime, _) = active_runtime(&mock);
    let first = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let second = runtime.open_by_serial(b"SECOND").unwrap();

    mock.push_close(Err(DriverError::Status(STATUS_CLOSE_FAILED)));
    assert!(first.close().is_err());
    let transfer = second
        .download_file(b"device-name.cfg", b"host-path.cfg")
        .unwrap();
    transfer.close().unwrap();

    assert_eq!(mock.closed_handles(), [FIRST_HANDLE, SECOND_HANDLE]);
    assert_eq!(operation_count(&mock, FfiOp::FileAccessRead), 1);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 2);
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
}

#[test]
fn post_entry_start_failure_closes_and_never_returns_the_device() {
    let mock = MockDriver::new();
    mock.push_file_access_read(Err(DriverError::Status(STATUS_CALL_FAILED)));
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let error =
        expect_error(device.download_file(b"secret-device-name.cfg", b"secret-host-path.cfg"));
    let names = take_only_file_name_lifetime();
    assert!(names.upgrade().is_none());
    assert!(matches!(error.start_error(), Error::Sdk { .. }));
    assert!(error.cleanup_error().is_none());
    assert_redacted(&error, &["secret-device-name.cfg", "secret-host-path.cfg"]);

    let error = match error.into_rejected_device() {
        Ok(_) => panic!("a post-entry failure returned its device"),
        Err(error) => error,
    };
    let (start, cleanup) = error.into_failed_after_driver_entry().unwrap();
    assert!(matches!(start, Error::Sdk { .. }));
    assert!(cleanup.is_none());
    assert_eq!(operation_count(&mock, FfiOp::FileAccessRead), 1);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn post_entry_start_and_close_failures_are_aggregated_and_names_are_leaked() {
    const HANDLE: usize = 0x3456_789A;

    let mock = MockDriver::new();
    mock.configure_open_by_ip(Some(HANDLE), Ok(()));
    mock.push_file_access_write(Err(DriverError::Status(STATUS_CALL_FAILED)));
    mock.push_close(Err(DriverError::Status(STATUS_CLOSE_FAILED)));
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    let error =
        expect_error(device.upload_file(b"secret-host-path.cfg", b"secret-device-name.cfg"));
    let names = take_only_file_name_lifetime();
    assert!(names.upgrade().is_some());
    let cleanup = error
        .cleanup_error()
        .expect("close failure must be retained");
    assert!(cleanup.stop.is_none());
    assert!(cleanup.close.is_some());
    assert_redacted(
        &error,
        &[
            "secret-device-name.cfg",
            "secret-host-path.cfg",
            "3456789a",
            "878082202",
        ],
    );

    let (start, cleanup) = error.into_failed_after_driver_entry().unwrap();
    assert!(matches!(start, Error::Sdk { .. }));
    let cleanup = cleanup.expect("cleanup failure was lost");
    assert!(cleanup.stop.is_none());
    assert!(cleanup.close.is_some());
    assert_eq!(operation_count(&mock, FfiOp::FileAccessWrite), 1);
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    assert_eq!(mock.closed_handles(), [HANDLE]);
    assert!(names.upgrade().is_some());
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 0,
            teardown_uncertain: true,
        })
    ));
}

#[derive(Clone, Copy, Debug)]
enum CleanupPath {
    DropRunning,
    DropCompleted,
    DropAfterProgressError,
    CloseRunning,
}

#[test]
fn transfer_drop_and_close_paths_close_once_without_stop() {
    for path in [
        CleanupPath::DropRunning,
        CleanupPath::DropCompleted,
        CleanupPath::DropAfterProgressError,
        CleanupPath::CloseRunning,
    ] {
        let mock = MockDriver::new();
        match path {
            CleanupPath::DropCompleted => {
                mock.push_file_access_progress(Ok(FileProgressRaw {
                    completed: 1,
                    total: 1,
                }));
            }
            CleanupPath::DropAfterProgressError => {
                mock.push_file_access_progress(Ok(FileProgressRaw {
                    completed: -1,
                    total: 1,
                }));
            }
            CleanupPath::DropRunning | CleanupPath::CloseRunning => {}
        }
        let (runtime, _) = active_runtime(&mock);
        let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
        let mut transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
        let names = take_only_file_name_lifetime();
        assert!(names.upgrade().is_some(), "{path:?}");

        match path {
            CleanupPath::DropRunning => drop(transfer),
            CleanupPath::DropCompleted => {
                assert!(matches!(
                    transfer.progress().unwrap(),
                    FileTransferStatus::Completed(_)
                ));
                drop(transfer);
            }
            CleanupPath::DropAfterProgressError => {
                assert!(transfer.progress().is_err());
                drop(transfer);
            }
            CleanupPath::CloseRunning => transfer.close().unwrap(),
        }

        assert!(names.upgrade().is_none(), "{path:?}");
        assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1, "{path:?}");
        assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0, "{path:?}");
        runtime.shutdown().unwrap();
    }
}

#[test]
fn completed_bundles_are_released_before_device_reuse() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 1,
        total: 1,
    }));
    let mut first = device
        .download_file(b"device-a.cfg", b"host-a.cfg")
        .unwrap();
    let first_names = take_only_file_name_lifetime();
    assert!(matches!(
        first.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
    assert!(first.retained_file_name_addresses_for_test().is_empty());
    assert!(first_names.upgrade().is_none());
    let (device, _) = match first.try_into_device() {
        Ok(recovered) => recovered,
        Err(_) => panic!("first completed transfer did not return its device"),
    };

    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 2,
        total: 2,
    }));
    let mut second = device.upload_file(b"host-b.cfg", b"device-b.cfg").unwrap();
    let second_names = take_only_file_name_lifetime();
    assert_eq!(second.direction(), FileTransferDirection::HostToDevice);
    assert!(matches!(
        second.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
    assert!(second.retained_file_name_addresses_for_test().is_empty());
    assert!(second_names.upgrade().is_none());
    let (device, _) = match second.try_into_device() {
        Ok(recovered) => recovered,
        Err(_) => panic!("second completed transfer did not return its device"),
    };
    assert!(device.retained_file_name_addresses_for_test().is_empty());
    device.close().unwrap();

    assert!(first_names.upgrade().is_none());
    assert!(second_names.upgrade().is_none());
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 1);
    assert_eq!(operation_count(&mock, FfiOp::StopMeasure), 0);
    runtime.shutdown().unwrap();
}

#[test]
fn close_failure_leaks_only_the_active_filename_bundle() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    mock.push_file_access_progress(Ok(FileProgressRaw {
        completed: 1,
        total: 1,
    }));
    let mut first = device
        .download_file(b"device-a.cfg", b"host-a.cfg")
        .unwrap();
    let first_names = take_only_file_name_lifetime();
    assert!(matches!(
        first.progress().unwrap(),
        FileTransferStatus::Completed(_)
    ));
    let (device, _) = match first.try_into_device() {
        Ok(recovered) => recovered,
        Err(_) => panic!("first completed transfer did not return its device"),
    };
    assert!(first_names.upgrade().is_none());

    let third = device
        .download_file(b"device-c.cfg", b"host-c.cfg")
        .unwrap();
    let third_names = take_only_file_name_lifetime();
    let third_addresses = file_access_addresses(&mock, 1);
    assert_eq!(
        third.retained_file_name_addresses_for_test(),
        [third_addresses]
    );

    mock.push_close(Err(DriverError::Status(STATUS_CLOSE_FAILED)));
    let cleanup = third.close().unwrap_err();
    assert!(cleanup.stop.is_none());
    assert!(cleanup.close.is_some());
    assert!(first_names.upgrade().is_none());
    assert!(third_names.upgrade().is_some());
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
fn forgetting_a_running_transfer_keeps_handle_and_names_live() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    let transfer = device.download_file(b"device.cfg", b"host.cfg").unwrap();
    let names = take_only_file_name_lifetime();

    std::mem::forget(transfer);
    assert!(names.upgrade().is_some());
    assert_eq!(operation_count(&mock, FfiOp::CloseDevice), 0);
    assert!(matches!(
        runtime.shutdown(),
        Err(Error::UnclosedDevices {
            live_handles: 1,
            teardown_uncertain: false,
        })
    ));
    assert!(names.upgrade().is_some());
    assert!(!mock.operations().contains(&FfiOp::Finalize));
}
