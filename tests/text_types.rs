use mv3d_lp::{CommandKey, Error, InputViolation, ParamKey, SdkText, SerialNumber};

// 验证 SDK 窄字符串按原始字节保存，防止非 UTF-8 内容被有损转换。
#[test]
fn sdk_text_keeps_non_utf8_bytes_without_loss() {
    let text = SdkText::new([0x66, 0x80, 0x6F]).unwrap();

    assert_eq!(text.as_bytes(), &[0x66, 0x80, 0x6F]);
    assert!(text.to_str().is_err());
    assert!(text.to_string_lossy().contains('\u{FFFD}'));
}

// 验证固定长度文本接受完整输出容量，防止边界值被提前拒绝。
#[test]
fn bounded_text_and_serial_number_accept_their_full_output_capacity() {
    assert_eq!(SdkText::new(vec![b'x'; 256]).unwrap().len(), 256);
    assert_eq!(SerialNumber::new(vec![b's'; 16]).unwrap().len(), 16);
}

// 验证 NUL 与非法 key 在 FFI 前被拒绝，防止 C 字符串截断或无效参数进入 SDK。
#[test]
fn nul_and_invalid_keys_are_rejected_before_ffi() {
    assert!(matches!(
        SerialNumber::new(b"a\0b"),
        Err(Error::InvalidInput {
            violation: InputViolation::InteriorNul,
            ..
        })
    ));
    assert!(matches!(
        ParamKey::new(""),
        Err(Error::InvalidInput {
            violation: InputViolation::Empty,
            ..
        })
    ));
    assert!(matches!(
        CommandKey::new("命令"),
        Err(Error::InvalidInput {
            violation: InputViolation::NonAscii,
            ..
        })
    ));
}

// 验证参数与命令 key 的字节上限，防止超过 SDK 固定缓冲区约定。
#[test]
fn parameter_and_command_keys_enforce_the_sdk_byte_limit() {
    let maximum_parameter_key = "p".repeat(ParamKey::MAX_LEN);
    let maximum_command_key = "c".repeat(CommandKey::MAX_LEN);
    assert_eq!(
        ParamKey::new(maximum_parameter_key)
            .unwrap()
            .as_bytes()
            .len(),
        ParamKey::MAX_LEN
    );
    assert_eq!(
        CommandKey::new(maximum_command_key)
            .unwrap()
            .as_bytes()
            .len(),
        CommandKey::MAX_LEN
    );

    assert!(matches!(
        ParamKey::new("p".repeat(ParamKey::MAX_LEN + 1)),
        Err(Error::InvalidInput {
            field: "parameter key",
            violation: InputViolation::TooLong {
                max: ParamKey::MAX_LEN,
                actual: 256,
            },
        })
    ));
    assert!(matches!(
        CommandKey::new("c".repeat(CommandKey::MAX_LEN + 1)),
        Err(Error::InvalidInput {
            field: "command key",
            violation: InputViolation::TooLong {
                max: CommandKey::MAX_LEN,
                actual: 256,
            },
        })
    ));
}
