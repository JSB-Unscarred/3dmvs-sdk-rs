use crate::device::{DeviceInfoRaw, DeviceRecord, bounded_bytes};

#[test]
fn fixed_sdk_text_is_copied_only_within_its_field() {
    assert_eq!(bounded_bytes(&[b'A', b'B', 0, b'C']), b"AB");
    assert_eq!(bounded_bytes(&[0xFF, 0xFE]), vec![0xFF, 0xFE]);
}

#[test]
fn converted_device_record_is_fully_owned() {
    let mut raw = DeviceInfoRaw::default();
    raw.model_name[..4].copy_from_slice(b"LNX\0");
    raw.serial_number[..4].copy_from_slice(b"SN01");
    raw.mac_address = [1, 2, 3, 4, 5, 6, 7, 8];
    raw.ip_configuration_mode = 2;
    let converted = DeviceRecord::from(raw);

    assert_eq!(converted.model_name, b"LNX");
    assert_eq!(converted.serial_number, b"SN01");
    assert_eq!(converted.mac_address, [1, 2, 3, 4, 5, 6, 7, 8]);
    assert_eq!(converted.ip_configuration_mode, 2);
}
