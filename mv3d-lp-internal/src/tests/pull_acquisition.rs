use std::net::Ipv4Addr;

use crate::driver::DriverError;
use crate::error::{Error, InvalidInput, Operation};
use crate::frame::{FrameRecord, ImageTypeRecord};
use crate::opened_device::DeviceState;

use super::mock_driver::{FfiOp, MockDriver, active_runtime};

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

// 验证 pull 采集路由 trigger、clear、get 与显式 stop，防止控制调用落到错误 handle 状态。
#[test]
fn device_routes_pull_controls_and_explicit_stop() {
    let mock = MockDriver::new();
    mock.push_get_image(Ok(frame(vec![1, 2, 3])));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    device.start().unwrap();
    device.soft_trigger().unwrap();
    device.clear_buffer().unwrap();
    let returned = device.get_image(37).unwrap();
    assert_eq!(returned.data, [1, 2, 3]);
    device.stop().unwrap();
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

// 验证 NO_DATA 精确保留且采集可重试，防止暂时缺帧被误作会话终止。
#[test]
fn no_data_is_exact_and_pull_acquisition_can_retry() {
    let mock = MockDriver::new();
    mock.push_get_image(Err(DriverError::Status(0x8006_0006_u32 as i32)));
    mock.push_get_image(Ok(frame(vec![9])));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();

    assert!(matches!(
        device.get_image(5),
        Err(Error::Sdk {
            operation: Operation::GetImage,
            status,
        }) if status as u32 == 0x8006_0006
    ));
    assert_eq!(device.get_image(6).unwrap().data, [9]);
    device.stop().unwrap();
    device.close().unwrap();

    assert_eq!(mock.image_timeouts(), [5, 6]);
}

// 验证断连状态码完整返回且清理继续执行，防止错误路径跳过 stop 和 close。
#[test]
fn disconnect_preserves_status_and_cleanup_still_runs() {
    let mock = MockDriver::new();
    mock.push_get_image(Err(DriverError::Status(0x8006_000D_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();

    assert!(matches!(
        device.get_image(25),
        Err(Error::Sdk {
            operation: Operation::GetImage,
            status,
        }) if status as u32 == 0x8006_000D
    ));
    device.stop().unwrap();
    device.close().unwrap();

    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
}

// 验证 active Device close 先 stop 再 close，防止活动采集遗留。
#[test]
fn active_device_stops_before_close() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    device.start().unwrap();
    assert_eq!(device.state(), DeviceState::Measuring);
    device.close().unwrap();

    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
}

// 验证显式 stop 失败使设备 Faulted 且 close 重试清理，防止不确定采集状态被复用。
#[test]
fn failed_explicit_stop_faults_device_and_close_retries_cleanup_stop() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    device.start().unwrap();
    assert!(matches!(device.stop(), Err(Error::Sdk { .. })));
    assert_eq!(device.state(), DeviceState::Faulted);
    device.close().unwrap();

    let operations = mock.operations();
    assert_eq!(
        &operations[operations.len() - 3..],
        [FfiOp::StopMeasure, FfiOp::StopMeasure, FfiOp::CloseDevice]
    );
}

// 验证 Device Drop 遇到 stop 失败仍继续 close，防止清理失败触发 unwind。
#[test]
fn failed_drop_stop_still_closes_once() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    device.start().unwrap();
    drop(device);

    let log = mock.logs();
    assert_eq!(&log[log.len() - 2..], ["stop", "close"]);
}

// 验证 SDK 无限超时 sentinel 在 driver 前被拒绝，防止安全 API 产生永久阻塞。
#[test]
fn infinite_timeout_sentinel_is_rejected_before_driver() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();

    assert!(matches!(
        device.get_image(u32::MAX),
        Err(Error::InvalidInput {
            kind: InvalidInput::TimeoutTooLong { .. },
            ..
        })
    ));
    assert!(mock.image_timeouts().is_empty());
    device.stop().unwrap();
    device.close().unwrap();
}

// 验证 trigger 与 clear 错误不结束采集，防止普通控制失败污染会话状态。
#[test]
fn trigger_and_clear_errors_do_not_end_the_session() {
    let mock = MockDriver::new();
    mock.push_soft_trigger(Err(DriverError::Status(0x8006_0007_u32 as i32)));
    mock.push_clear_buffer(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    device.start().unwrap();

    assert!(matches!(device.soft_trigger(), Err(Error::Sdk { .. })));
    assert!(matches!(device.clear_buffer(), Err(Error::Sdk { .. })));
    assert_eq!(device.state(), DeviceState::Measuring);
    device.stop().unwrap();
    device.close().unwrap();
}
