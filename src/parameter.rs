//! 设备参数：`MV3D_LP_PARAM` 的 tagged union 与 Rust enum 之间的转换。

use std::ffi::CString;

use crate::{Error, ErrorCode, Result, fixed_cstr, sys};

/// [`Device::get_parameter`](crate::Device::get_parameter) 读到的参数值与约束。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Parameter {
    /// Boolean。
    Bool(bool),
    /// Integer，附带取值约束。
    Integer {
        /// 当前值。
        value: i64,
        /// 最小值。
        min: i64,
        /// 最大值。
        max: i64,
        /// 步长。
        increment: i64,
    },
    /// Float，附带取值范围。
    Float {
        /// 当前值。
        value: f32,
        /// 最小值。
        min: f32,
        /// 最大值。
        max: f32,
    },
    /// Enumeration，附带候选值。
    Enumeration {
        /// 当前值。
        value: u32,
        /// 节点支持的全部值。
        supported: Vec<u32>,
    },
    /// String，附带字段容量。
    String {
        /// 当前值。
        value: CString,
        /// 最大字节数。
        max_length: u32,
    },
}

/// [`Device::set_parameter`](crate::Device::set_parameter) 写入的参数值。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParameterValue {
    /// Boolean。
    Bool(bool),
    /// Integer。
    Integer(i64),
    /// Float。
    Float(f32),
    /// Enumeration。
    Enumeration(u32),
    /// String，最多 255 字节。
    String(CString),
}

impl Parameter {
    /// 按类型字段读取 union 成员；SDK 返回未知类型时报告 `UnknownGeneric`。
    pub(crate) fn from_raw(raw: &sys::MV3D_LP_PARAM) -> Result<Self> {
        let info = &raw.ParamInfo;
        // SAFETY: 每个分支只读取 enParamType 指明的 union 成员。
        unsafe {
            Ok(match raw.enParamType {
                sys::ParamType_Bool => Self::Bool(info.bBoolParam != 0),
                sys::ParamType_Int => {
                    let int = &info.stIntParam;
                    Self::Integer {
                        value: int.nCurValue,
                        min: int.nMin,
                        max: int.nMax,
                        increment: int.nInc,
                    }
                }
                sys::ParamType_Float => {
                    let float = &info.stFloatParam;
                    Self::Float {
                        value: float.fCurValue,
                        min: float.fMin,
                        max: float.fMax,
                    }
                }
                sys::ParamType_Enum => {
                    let enumeration = &info.stEnumParam;
                    let count =
                        (enumeration.nSupportedNum as usize).min(enumeration.nSupportValue.len());
                    Self::Enumeration {
                        value: enumeration.nCurValue,
                        supported: enumeration.nSupportValue[..count].to_vec(),
                    }
                }
                sys::ParamType_String => {
                    let string = &info.stStringParam;
                    Self::String {
                        value: fixed_cstr(&string.chCurValue).to_owned(),
                        max_length: string.nMaxLength,
                    }
                }
                _ => {
                    return Err(Error::Sdk {
                        function: "MV3D_LP_GetParam",
                        code: ErrorCode::UnknownGeneric,
                    });
                }
            })
        }
    }
}

impl ParameterValue {
    /// 构造 SDK 结构体；其余 union 字节与保留字段保持为零。
    pub(crate) fn to_raw(&self) -> Result<sys::MV3D_LP_PARAM> {
        let mut raw = sys::MV3D_LP_PARAM::default();
        let info = &mut raw.ParamInfo;
        raw.enParamType = match self {
            Self::Bool(value) => {
                info.bBoolParam = sys::BOOL::from(*value);
                sys::ParamType_Bool
            }
            Self::Integer(value) => {
                info.stIntParam.nCurValue = *value;
                sys::ParamType_Int
            }
            Self::Float(value) => {
                info.stFloatParam.fCurValue = *value;
                sys::ParamType_Float
            }
            Self::Enumeration(value) => {
                info.stEnumParam.nCurValue = *value;
                sys::ParamType_Enum
            }
            Self::String(value) => {
                let bytes = value.as_bytes_with_nul();
                // SAFETY: 写入 stStringParam 成员，union 已整体清零。
                let field = unsafe { &mut info.stStringParam.chCurValue };
                if bytes.len() > field.len() {
                    return Err(Error::InvalidInput("字符串参数超过 255 字节"));
                }
                for (target, byte) in field.iter_mut().zip(bytes) {
                    *target = byte.cast_signed();
                }
                sys::ParamType_String
            }
        };
        Ok(raw)
    }
}

#[cfg(test)]
mod tests {
    use super::{Parameter, ParameterValue};
    use crate::{Error, sys};

    // union 按类型字段读写；枚举候选数按字段容量截断，超长字符串在调用 SDK 前被拒绝。
    #[test]
    fn tagged_union_round_trip() {
        let mut raw = ParameterValue::Enumeration(7).to_raw().unwrap();
        // SAFETY: to_raw 已按 Enumeration 写入 stEnumParam。
        unsafe {
            raw.ParamInfo.stEnumParam.nSupportedNum = 99;
            raw.ParamInfo.stEnumParam.nSupportValue[1] = 3;
        }
        match Parameter::from_raw(&raw).unwrap() {
            Parameter::Enumeration { value, supported } => {
                assert_eq!(value, 7);
                assert_eq!(
                    (supported.len(), supported[1]),
                    (sys::MV3D_LP_MAX_ENUM_COUNT, 3)
                );
            }
            other => panic!("unexpected {other:?}"),
        }

        let raw = ParameterValue::String(c"abc".to_owned()).to_raw().unwrap();
        assert!(
            matches!(Parameter::from_raw(&raw), Ok(Parameter::String { value, .. }) if value.as_bytes() == b"abc")
        );

        let long = ParameterValue::String(std::ffi::CString::new(vec![b'a'; 256]).unwrap());
        assert!(matches!(long.to_raw(), Err(Error::InvalidInput(_))));
    }
}
