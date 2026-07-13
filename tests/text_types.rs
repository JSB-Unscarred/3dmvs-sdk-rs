use mv3d_lp::{CommandKey, Error, InputViolation, ParamKey, SdkText, SerialNumber};

#[test]
fn sdk_text_keeps_non_utf8_bytes_without_loss() {
    let text = SdkText::new([0x66, 0x80, 0x6F]).unwrap();

    assert_eq!(text.as_bytes(), &[0x66, 0x80, 0x6F]);
    assert!(text.to_str().is_err());
    assert!(text.to_string_lossy().contains('\u{FFFD}'));
}

#[test]
fn bounded_text_and_serial_number_accept_their_full_output_capacity() {
    assert_eq!(SdkText::new(vec![b'x'; 256]).unwrap().len(), 256);
    assert_eq!(SerialNumber::new(vec![b's'; 16]).unwrap().len(), 16);
}

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
