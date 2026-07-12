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
