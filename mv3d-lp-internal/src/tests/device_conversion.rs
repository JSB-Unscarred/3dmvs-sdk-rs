use crate::device::{DeviceInfoRaw, DeviceRecord};

// 验证固定字段按边界截断，并复制为 owned 设备记录。
#[test]
fn device_record_owns_bounded_sdk_fields() {
    let mut raw = DeviceInfoRaw::default();
    raw.model_name[..5].copy_from_slice(b"LNX\0X");
    raw.serial_number[..4].copy_from_slice(b"SN01");
    let converted = DeviceRecord::from(raw);

    assert_eq!(converted.model_name, b"LNX");
    assert_eq!(converted.serial_number, b"SN01");
}
