use crate::{Error, InputViolation, Result};
use std::borrow::Cow;
use std::fmt;
use std::str::{FromStr, Utf8Error};

/// SDK-originated text kept as bounded bytes without assuming an encoding.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SdkText(Vec<u8>);

impl SdkText {
    /// Size of the largest fixed SDK string storage, including its terminator.
    pub const STORAGE_CAPACITY: usize = 256;

    /// Maximum bounded payload copied from SDK-owned fixed storage.
    ///
    /// Setters impose their own 255-byte limit so that they can append a NUL.
    pub const MAX_LEN: usize = Self::STORAGE_CAPACITY;

    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self> {
        Self::from_vec(bytes.as_ref().to_vec())
    }

    fn from_vec(bytes: Vec<u8>) -> Result<Self> {
        validate_bytes("SDK text", &bytes, Self::MAX_LEN)?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_str(&self) -> std::result::Result<&str, Utf8Error> {
        std::str::from_utf8(&self.0)
    }

    #[must_use]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for SdkText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SdkText")
            .field("bytes", &self.0)
            .field("lossy", &self.to_string_lossy())
            .finish()
    }
}

impl fmt::Display for SdkText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_string_lossy())
    }
}

impl AsRef<[u8]> for SdkText {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl TryFrom<&[u8]> for SdkText {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<Vec<u8>> for SdkText {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self> {
        Self::from_vec(value)
    }
}

impl TryFrom<&str> for SdkText {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value.as_bytes())
    }
}

impl TryFrom<String> for SdkText {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_vec(value.into_bytes())
    }
}

impl FromStr for SdkText {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value)
    }
}

/// A serial number represented as at most 16 non-NUL bytes.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SerialNumber(Vec<u8>);

impl SerialNumber {
    pub const MAX_LEN: usize = 16;

    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self> {
        Self::from_vec(bytes.as_ref().to_vec())
    }

    fn from_vec(bytes: Vec<u8>) -> Result<Self> {
        validate_bytes("serial number", &bytes, Self::MAX_LEN)?;
        Ok(Self(bytes))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn to_str(&self) -> std::result::Result<&str, Utf8Error> {
        std::str::from_utf8(&self.0)
    }

    #[must_use]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.0)
    }

    #[must_use]
    pub fn into_bytes(self) -> Vec<u8> {
        self.0
    }
}

impl fmt::Debug for SerialNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SerialNumber")
            .field("bytes", &self.0)
            .field("lossy", &self.to_string_lossy())
            .finish()
    }
}

impl fmt::Display for SerialNumber {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.to_string_lossy())
    }
}

impl AsRef<[u8]> for SerialNumber {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl TryFrom<&[u8]> for SerialNumber {
    type Error = Error;

    fn try_from(value: &[u8]) -> Result<Self> {
        Self::new(value)
    }
}

impl TryFrom<Vec<u8>> for SerialNumber {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self> {
        Self::from_vec(value)
    }
}

impl TryFrom<&str> for SerialNumber {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self> {
        Self::new(value.as_bytes())
    }
}

impl TryFrom<String> for SerialNumber {
    type Error = Error;

    fn try_from(value: String) -> Result<Self> {
        Self::from_vec(value.into_bytes())
    }
}

impl FromStr for SerialNumber {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value)
    }
}

fn validate_bytes(field: &'static str, bytes: &[u8], max: usize) -> Result<()> {
    if bytes.len() > max {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::TooLong {
                max,
                actual: bytes.len(),
            },
        });
    }

    if bytes.contains(&0) {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::InteriorNul,
        });
    }

    Ok(())
}

macro_rules! ascii_key {
    ($name:ident, $field:literal) => {
        #[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(String);

        impl $name {
            /// Maximum key length accepted by the SDK, in bytes.
            pub const MAX_LEN: usize = 255;

            pub fn new(value: impl Into<String>) -> Result<Self> {
                let value = value.into();
                validate_ascii_key($field, &value, Self::MAX_LEN)?;
                Ok(Self(value))
            }

            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }

            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                self.0.as_bytes()
            }

            #[must_use]
            pub fn into_string(self) -> String {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                self.as_str()
            }
        }

        impl TryFrom<&str> for $name {
            type Error = Error;

            fn try_from(value: &str) -> Result<Self> {
                Self::new(value)
            }
        }

        impl TryFrom<String> for $name {
            type Error = Error;

            fn try_from(value: String) -> Result<Self> {
                Self::new(value)
            }
        }

        impl FromStr for $name {
            type Err = Error;

            fn from_str(value: &str) -> Result<Self> {
                Self::new(value)
            }
        }
    };
}

ascii_key!(ParamKey, "parameter key");
ascii_key!(CommandKey, "command key");

fn validate_ascii_key(field: &'static str, value: &str, max: usize) -> Result<()> {
    if value.is_empty() {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::Empty,
        });
    }

    if value.len() > max {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::TooLong {
                max,
                actual: value.len(),
            },
        });
    }

    if value.as_bytes().contains(&0) {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::InteriorNul,
        });
    }

    if !value.is_ascii() {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::NonAscii,
        });
    }

    Ok(())
}
