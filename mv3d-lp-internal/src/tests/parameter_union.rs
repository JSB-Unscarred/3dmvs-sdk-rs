use crate::bindings::{MV3D_LP_ENUMPARAM, ParamType_Enum};
use crate::error::ContractViolation;
use crate::ffi::{
    bool_parameter_has_zeroed_inactive_storage, parameter_from_native, parameter_to_native,
    zeroed_parameter,
};
use crate::parameter::{ParameterRecord, ParameterValueRecord};

// 验证 discriminator 选择 union 成员，并在切片前校验固定数组计数。
#[test]
fn parameter_union_uses_its_tag_and_bounds_enum_count() {
    let mut parameter = enumeration_parameter(2);
    assert_eq!(
        parameter_from_native(&parameter).unwrap(),
        ParameterRecord::Enumeration {
            value: 7,
            supported: vec![1, 7],
        }
    );

    parameter.ParamInfo.stEnumParam.nSupportedNum = 17;
    assert!(matches!(
        parameter_from_native(&parameter),
        Err(crate::driver::DriverError::Contract(
            ContractViolation::EnumCountExceedsLimit { .. }
        ))
    ));
}

// 验证 setter 清零 reserved 与未激活 union storage。
#[test]
fn parameter_setter_initializes_the_whole_native_value() {
    let parameter = parameter_to_native(&ParameterValueRecord::Bool(true)).unwrap();
    assert!(bool_parameter_has_zeroed_inactive_storage(&parameter));
}

fn enumeration_parameter(count: u32) -> crate::bindings::MV3D_LP_PARAM {
    let mut parameter = zeroed_parameter();
    parameter.enParamType = ParamType_Enum;
    parameter.ParamInfo.stEnumParam = MV3D_LP_ENUMPARAM {
        nCurValue: 7,
        nSupportedNum: count,
        nSupportValue: [1, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };
    parameter
}
