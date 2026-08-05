use crate::device::{DeviceInfoRaw, DeviceListAttempt};
use crate::error::{ContractViolation, Error};

use super::mock_driver::{MockDriver, active_runtime};

// 验证设备列表增长时按新容量重试，防止枚举期间设备变化造成结果截断。
#[test]
fn discovery_retries_when_the_device_list_grows() {
    let mock = MockDriver::new();
    mock.push_device_number(Ok(1));
    mock.push_device_list(Ok(DeviceListAttempt {
        records: vec![DeviceInfoRaw::default()],
        reported: 2,
    }));
    mock.push_device_list(Ok(DeviceListAttempt {
        records: vec![DeviceInfoRaw::default(), DeviceInfoRaw::default()],
        reported: 2,
    }));
    let (runtime, _) = active_runtime(&mock);

    assert_eq!(runtime.devices().unwrap().len(), 2);
    assert_eq!(mock.capacities(), vec![1, 2]);
}

// 验证异常大的设备数量在分配前被拒绝，防止不可信 SDK 计数触发巨额分配。
#[test]
fn discovery_rejects_unbounded_sdk_counts() {
    let mock = MockDriver::new();
    mock.push_device_number(Ok(257));
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.devices(),
        Err(Error::ContractViolation {
            kind: ContractViolation::DeviceCountExceedsLimit { .. },
            ..
        })
    ));
    assert!(mock.capacities().is_empty());
}

// 验证连续不稳定快照按上限终止，防止设备变化导致枚举无限重试。
#[test]
fn discovery_stops_after_three_unstable_snapshots() {
    let mock = MockDriver::new();
    mock.push_device_number(Ok(1));
    for reported in [2, 5, 11] {
        mock.push_device_list(Ok(DeviceListAttempt {
            records: vec![DeviceInfoRaw::default(); reported - 1],
            reported: reported as u32,
        }));
    }
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.devices(),
        Err(Error::DiscoveryChanged { attempts: 3 })
    ));
}
