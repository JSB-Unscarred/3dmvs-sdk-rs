use crate::SdkText;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ParameterKind {
    Bool,
    Integer,
    Float,
    Enumeration,
    String,
}

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
    #[must_use]
    pub const fn kind(&self) -> ParameterKind {
        match self {
            Self::Bool(_) => ParameterKind::Bool,
            Self::Integer { .. } => ParameterKind::Integer,
            Self::Float { .. } => ParameterKind::Float,
            Self::Enumeration { .. } => ParameterKind::Enumeration,
            Self::String { .. } => ParameterKind::String,
        }
    }

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

impl ParameterValue {
    #[must_use]
    pub const fn kind(&self) -> ParameterKind {
        match self {
            Self::Bool(_) => ParameterKind::Bool,
            Self::Integer(_) => ParameterKind::Integer,
            Self::Float(_) => ParameterKind::Float,
            Self::Enumeration(_) => ParameterKind::Enumeration,
            Self::String(_) => ParameterKind::String,
        }
    }
}
