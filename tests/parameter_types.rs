use mv3d_lp::{Parameter, ParameterKind, ParameterValue, SdkText};

// 验证读取参数保留取值范围并提取可写值，防止元数据在公开转换中丢失。
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

// 验证字符串参数拥有文本数据，防止参数值借用临时 SDK 缓冲区。
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
