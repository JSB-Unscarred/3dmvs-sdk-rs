use crate::device::{DeviceInfoRaw, DeviceListAttempt};

use super::mock_driver::{MockDriver, active_runtime};

// 验证一次数量查询和一次列表查询返回 owned 记录。
#[test]
fn discovery_returns_the_reported_owned_records() {
    let mock = MockDriver::new();
    mock.push_device_number(Ok(1));
    let mut record = DeviceInfoRaw::default();
    record.serial_number[..4].copy_from_slice(b"SN02");
    mock.push_device_list(Ok(DeviceListAttempt {
        records: vec![record],
        reported: 1,
    }));
    let (runtime, _) = active_runtime(&mock);

    let devices = runtime.devices().unwrap();

    assert_eq!(mock.capacities(), [1]);
    assert_eq!(devices.len(), 1);
    assert_eq!(devices[0].serial_number, b"SN02");
}
