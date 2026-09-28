//! 设备参数：`MV3D_LP_PARAM` 的 tagged union 与 Rust enum 之间的转换。

use std::ffi::{CStr, CString};

use crate::{Error, Result, fixed_cstring, sys};

/// [`Device::get_parameter`](crate::Device::get_parameter) 读到的参数值与约束。
///
/// 变体名对应 SDK 的 `ParamType_*`，字段名与 mvs-sdk 的 `IntValue` 等节点值类型一致。
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Parameter {
    /// Boolean。
    Bool(bool),
    /// Integer，附带取值约束。
    #[non_exhaustive]
    Int {
        /// 当前值。
        current: i64,
        /// 最小值。
        min: i64,
        /// 最大值。
        max: i64,
        /// 步长。
        increment: i64,
    },
    /// Float，附带取值范围。
    #[non_exhaustive]
    Float {
        /// 当前值。
        current: f32,
        /// 最小值。
        min: f32,
        /// 最大值。
        max: f32,
    },
    /// Enumeration，附带候选值。
    #[non_exhaustive]
    Enum {
        /// 当前值。
        current: u32,
        /// 节点支持的全部值。
        supported: Vec<u32>,
    },
    /// String，附带字段容量。
    #[non_exhaustive]
    String {
        /// 当前值，保留 SDK 原始字节。
        current: CString,
        /// 最大字节数。
        max_length: u32,
    },
    /// 头文件未定义的参数类型，保存原始 `enParamType`；值无法解读。
    Other(i32),
}

/// [`Device::set_parameter`](crate::Device::set_parameter) 写入的参数值。
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParameterValue<'a> {
    /// Boolean。
    Bool(bool),
    /// Integer。
    Int(i64),
    /// Float。
    Float(f32),
    /// Enumeration。
    Enum(u32),
    /// String，最多 255 字节。
    String(&'a CStr),
}

impl Parameter {
    /// 按类型字段读取 union 成员；未知类型保存在 [`Parameter::Other`]。
    pub(crate) fn from_raw(raw: &sys::MV3D_LP_PARAM) -> Self {
        let info = &raw.ParamInfo;
        // SAFETY: 每个分支只读取 enParamType 指明的 union 成员。
        unsafe {
            match raw.enParamType {
                sys::ParamType_Bool => Self::Bool(info.bBoolParam != 0),
                sys::ParamType_Int => {
                    let int = &info.stIntParam;
                    Self::Int {
                        current: int.nCurValue,
                        min: int.nMin,
                        max: int.nMax,
                        increment: int.nInc,
                    }
                }
                sys::ParamType_Float => {
                    let float = &info.stFloatParam;
                    Self::Float {
                        current: float.fCurValue,
                        min: float.fMin,
                        max: float.fMax,
                    }
                }
                sys::ParamType_Enum => {
                    let enumeration = &info.stEnumParam;
                    let count =
                        (enumeration.nSupportedNum as usize).min(enumeration.nSupportValue.len());
                    Self::Enum {
                        current: enumeration.nCurValue,
                        supported: enumeration.nSupportValue[..count].to_vec(),
                    }
                }
                sys::ParamType_String => {
                    let string = &info.stStringParam;
                    Self::String {
                        current: fixed_cstring(&string.chCurValue),
                        max_length: string.nMaxLength,
                    }
                }
                other => Self::Other(other),
            }
        }
    }
}

impl ParameterValue<'_> {
    /// 构造 SDK 结构体；其余 union 字节与保留字段保持为零。
    pub(crate) fn to_raw(self) -> Result<sys::MV3D_LP_PARAM> {
        let mut raw = sys::MV3D_LP_PARAM::default();
        let info = &mut raw.ParamInfo;
        raw.enParamType = match self {
            Self::Bool(value) => {
                info.bBoolParam = sys::BOOL::from(value);
                sys::ParamType_Bool
            }
            Self::Int(value) => {
                info.stIntParam.nCurValue = value;
                sys::ParamType_Int
            }
            Self::Float(value) => {
                info.stFloatParam.fCurValue = value;
                sys::ParamType_Float
            }
            Self::Enum(value) => {
                info.stEnumParam.nCurValue = value;
                sys::ParamType_Enum
            }
            Self::String(value) => {
                let bytes = value.to_bytes_with_nul();
                // SAFETY: 写入 stStringParam 成员，union 已整体清零。
                let field = unsafe { &mut info.stStringParam.chCurValue };
                if bytes.len() > field.len() {
                    return Err(Error::InvalidInput("string parameter exceeds 255 bytes"));
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
