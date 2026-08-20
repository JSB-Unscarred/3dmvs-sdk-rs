use crate::text::SdkText;

/// An owned parameter value together with the limits reported by the SDK.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Parameter {
    Bool(bool),
    Integer {
        value: i64,
        min: i64,
        max: i64,
        increment: i64,
    },
    Float {
        value: f32,
        min: f32,
        max: f32,
    },
    Enumeration {
        value: u32,
        supported: Vec<u32>,
    },
    String {
        value: SdkText,
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
    Bool(bool),
    Integer(i64),
    Float(f32),
    Enumeration(u32),
    String(SdkText),
}
