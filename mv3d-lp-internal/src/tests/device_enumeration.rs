use crate::device::{DeviceInfoRaw, DeviceListAttempt};
use crate::error::{ContractViolation, Error};

use super::mock_driver::{MockDriver, active_runtime};

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
