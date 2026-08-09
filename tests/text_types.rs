use mv3d_lp::{CommandKey, Error, InputViolation, ParamKey, SdkText, SerialNumber};

// 验证 SDK 文本按原始字节保存，并接受固定缓冲区的完整输出容量。
#[test]
fn sdk_text_is_lossless_and_accepts_full_capacity() {
    let text = SdkText::new([0x66, 0x80, 0x6F]).unwrap();
    assert_eq!(text.as_bytes(), &[0x66, 0x80, 0x6F]);
    assert!(text.to_str().is_err());
    assert_eq!(SdkText::new(vec![b'x'; 256]).unwrap().len(), 256);
    assert_eq!(SerialNumber::new(vec![b's'; 16]).unwrap().len(), 16);
}

// 验证固定字段与 C 字符串边界，同时让 SDK 判断节点名是否存在。
#[test]
fn ffi_text_inputs_reject_invalid_bytes_and_lengths() {
    assert!(matches!(
        SerialNumber::new(b"a\0b"),
        Err(Error::InvalidInput {
            violation: InputViolation::InteriorNul,
            ..
        })
    ));
    assert!(CommandKey::new("命令").is_ok());
    assert!(ParamKey::new("p".repeat(1024)).is_ok());
}
