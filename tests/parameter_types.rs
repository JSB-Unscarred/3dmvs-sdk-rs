use mv3d_lp::{Parameter, ParameterKind, ParameterValue, SdkText};

#[test]
fn read_parameter_retains_limits_and_extracts_a_settable_value() {
    let parameter = Parameter::Integer {
        value: 640,
        min: 32,
        max: 4096,
        increment: 8,
    };

    assert_eq!(parameter.kind(), ParameterKind::Integer);
    assert_eq!(parameter.value(), ParameterValue::Integer(640));
}

#[test]
fn string_parameter_is_owned() {
    let value = SdkText::new(b"profile").unwrap();
    let parameter = Parameter::String {
        value: value.clone(),
        max_length: 64,
    };

    assert_eq!(parameter.kind(), ParameterKind::String);
    assert_eq!(parameter.value(), ParameterValue::String(value));
}
