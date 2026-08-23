use crate::text::SdkText;

/// An owned parameter value together with the limits reported by the SDK.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Parameter {
    /// 布尔参数。
    Bool(bool),
    /// 整数参数，附带 SDK 报告的取值范围与步进。
    Integer {
        /// 当前值。
        value: i64,
        /// 允许的最小值。
        min: i64,
        /// 允许的最大值。
        max: i64,
        /// 取值步进。
        increment: i64,
    },
    /// 浮点参数，附带 SDK 报告的取值范围。
    Float {
        /// 当前值。
        value: f32,
        /// 允许的最小值。
        min: f32,
        /// 允许的最大值。
        max: f32,
    },
    /// 枚举参数，附带 SDK 报告的可选值。
    Enumeration {
        /// 当前值。
        value: u32,
        /// 该节点支持的全部取值。
        supported: Vec<u32>,
    },
    /// 字符串参数，附带 SDK 字段容量。
    String {
        /// 当前值。
        value: SdkText,
        /// 该字段允许的最大字节数。
        max_length: u32,
    },
}

impl Parameter {
    /// Extracts the current value without the limits returned by `GetParam`.
    #[must_use]
    pub fn value(&self) -> ParameterValue {
        match self {
            Self::Bool(value) => ParameterValue::Bool(*value),
            Self::Integer { value, .. } => ParameterValue::Integer(*value),
            Self::Float { value, .. } => ParameterValue::Float(*value),
            Self::Enumeration { value, .. } => ParameterValue::Enumeration(*value),
            Self::String { value, .. } => ParameterValue::String(value.clone()),
        }
    }
}

/// A value accepted by the SDK's parameter-setting operation.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum ParameterValue {
    /// 布尔值。
    Bool(bool),
    /// 整数值。
    Integer(i64),
    /// 浮点值。
    Float(f32),
    /// 枚举值。
    Enumeration(u32),
    /// 字符串值。
    String(SdkText),
}
