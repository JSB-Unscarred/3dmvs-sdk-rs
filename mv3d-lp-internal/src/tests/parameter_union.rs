use crate::bindings::{MV3D_LP_ENUMPARAM, ParamType_Bool, ParamType_Enum};
use crate::error::ContractViolation;
use crate::ffi::{
    bool_parameter_has_zeroed_inactive_storage, parameter_from_native, parameter_to_native,
    zeroed_parameter,
};
use crate::parameter::{ParameterRecord, ParameterValueRecord};

use super::mock_driver::{MockDriver, active_runtime};

// 验证已知 discriminator 只读取对应 union 字段，防止访问未激活成员。
#[test]
fn tagged_union_is_read_only_after_a_known_discriminator() {
    let mut parameter = zeroed_parameter();
    parameter.enParamType = ParamType_Enum;
    parameter.ParamInfo.stEnumParam = MV3D_LP_ENUMPARAM {
        nCurValue: 7,
        nSupportedNum: 2,
        nSupportValue: [1, 7, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    };

    assert_eq!(
        parameter_from_native(&parameter).unwrap(),
        ParameterRecord::Enumeration {
            value: 7,
            supported: vec![1, 7],
        }
    );
}

// 验证超大枚举计数在切片构造前被拒绝，防止越界读取 SDK 数组。
#[test]
fn oversized_enum_count_is_rejected_before_slicing() {
    let mut parameter = zeroed_parameter();
    parameter.enParamType = ParamType_Enum;
    parameter.ParamInfo.stEnumParam = MV3D_LP_ENUMPARAM {
        nCurValue: 0,
        nSupportedNum: 17,
        nSupportValue: [0; 16],
    };

    assert!(matches!(
        parameter_from_native(&parameter),
        Err(crate::driver::DriverError::Contract(
            ContractViolation::EnumCountExceedsLimit { .. }
        ))
    ));
}

// 验证 SDK bool 的任意非零值映射为 true，防止假定厂商只返回一。
#[test]
fn any_nonzero_sdk_bool_is_true() {
    let mut parameter = zeroed_parameter();
    parameter.enParamType = ParamType_Bool;
    parameter.ParamInfo.bBoolParam = -1;

    assert_eq!(
        parameter_from_native(&parameter).unwrap(),
        ParameterRecord::Bool(true)
    );
}

// 验证 setter 清零 reserved 与未激活 union 存储，防止未初始化字节进入 FFI。
#[test]
fn setter_zeroes_reserved_and_inactive_union_storage() {
    let parameter = parameter_to_native(&ParameterValueRecord::Bool(true)).unwrap();
    assert!(bool_parameter_has_zeroed_inactive_storage(&parameter));
}

// 验证参数记录以 owned 安全类型穿过 mock 边界，防止 raw union 泄漏到 driver API。
#[test]
fn owned_parameter_record_crosses_the_mock_boundary_without_raw_union_data() {
    let mock = MockDriver::new();
    mock.push_get_parameter(Ok(ParameterRecord::Integer {
        value: 8,
        minimum: 1,
        maximum: 16,
        increment: 1,
    }));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(std::net::Ipv4Addr::LOCALHOST).unwrap();

    assert_eq!(
        device.get_parameter(b"Width").unwrap(),
        ParameterRecord::Integer {
            value: 8,
            minimum: 1,
            maximum: 16,
            increment: 1,
        }
    );
}
