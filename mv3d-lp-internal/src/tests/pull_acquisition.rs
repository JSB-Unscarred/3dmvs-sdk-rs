use std::net::Ipv4Addr;

use crate::frame::{FrameRecord, ImageTypeRecord};

use super::mock_driver::{Call, MockDriver, active_runtime};

// 验证 open→start→pull→stop→close→shutdown 的标准数据流。
#[test]
fn standard_pull_lifecycle_routes_one_owned_frame() {
    let mock = MockDriver::new();
    mock.push_get_image(Ok(frame(vec![1, 2, 3])));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    device.start().unwrap();
    device.soft_trigger().unwrap();
    device.clear_buffer().unwrap();
    assert_eq!(device.get_image(37).unwrap().data, [1, 2, 3]);
    device.stop().unwrap();
    device.close().unwrap();
    runtime.shutdown().unwrap();

    assert_eq!(mock.image_timeouts(), [37]);
    assert_eq!(
        mock.calls(),
        [
            Call::Version,
            Call::Initialize,
            Call::OpenByIp,
            Call::Start,
            Call::SoftTrigger,
            Call::ClearBuffer,
            Call::GetImage,
            Call::Stop,
            Call::Close,
            Call::Finalize,
        ]
    );
}

fn frame(data: Vec<u8>) -> FrameRecord {
    FrameRecord {
        image_type: ImageTypeRecord::from_bits(0x0108_0001),
        width: data.len() as u32,
        height: 1,
        data,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 7,
        device_timestamp: 11,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    }
}
