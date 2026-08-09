use crate::error::{ContractViolation, Error, InvalidInput};
use crate::file_transfer::{FileProgressRaw, FileTransferStatus};
use crate::opened_device::DeviceState;

use super::mock_driver::{Call, MockDriver, active_runtime};

// 验证下载/上传的文件名顺序以及完成后设备复用。
#[test]
fn file_transfer_routes_both_directions_and_completes() {
    let mock = MockDriver::new();
    for total in [4, 7] {
        mock.push_file_progress(Ok(FileProgressRaw {
            completed: total,
            total,
        }));
    }
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    device.download_file(b"device.cfg", b"host.cfg").unwrap();
    assert!(matches!(
        device.file_transfer_progress().unwrap(),
        FileTransferStatus::Completed(progress) if progress.total == 4
    ));
    device.upload_file(b"next.bin", b"device.bin").unwrap();
    assert!(matches!(
        device.file_transfer_progress().unwrap(),
        FileTransferStatus::Completed(progress) if progress.total == 7
    ));
    assert_eq!(device.state(), DeviceState::Open);

    let calls = mock.file_calls();
    assert_eq!(calls[0].direction, Call::FileRead);
    assert_eq!(calls[0].user_file_name, b"host.cfg");
    assert_eq!(calls[0].device_file_name, b"device.cfg");
    assert_eq!(calls[1].direction, Call::FileWrite);
    assert_eq!(calls[1].user_file_name, b"next.bin");
    assert_eq!(calls[1].device_file_name, b"device.bin");
}

// 验证非法快照不会结束传输，下一次合法快照仍可完成。
#[test]
fn invalid_progress_snapshot_is_retryable() {
    let mock = MockDriver::new();
    mock.push_file_progress(Ok(FileProgressRaw {
        completed: -1,
        total: 10,
    }));
    mock.push_file_progress(Ok(FileProgressRaw {
        completed: 10,
        total: 10,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    device.download_file(b"device.cfg", b"host.cfg").unwrap();

    assert!(matches!(
        device.file_transfer_progress(),
        Err(Error::ContractViolation {
            kind: ContractViolation::NegativeFileProgress { .. },
            ..
        })
    ));
    assert_eq!(device.state(), DeviceState::Transferring);
    assert!(matches!(
        device.file_transfer_progress(),
        Ok(FileTransferStatus::Completed(_))
    ));
}

// 验证空值与 NUL 在 driver 前拒绝。
#[test]
fn invalid_file_names_do_not_reach_the_driver() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();

    assert!(matches!(
        device.download_file(b"", b"host.cfg"),
        Err(Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        })
    ));
    assert!(matches!(
        device.upload_file(b"bad\0name", b"device.cfg"),
        Err(Error::InvalidInput {
            kind: InvalidInput::InteriorNul,
            ..
        })
    ));
    assert!(mock.file_calls().is_empty());
}
