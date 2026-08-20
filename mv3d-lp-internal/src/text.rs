use std::borrow::Cow;
use std::fmt;
use std::str::{FromStr, Utf8Error};

use crate::cstr::c_string;
use crate::error::{Error, InputViolation};

/// SDK-originated text kept as owned bytes without assuming an encoding.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SdkText(Vec<u8>);

impl SdkText {
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        let bytes = bytes.as_ref();
        reject_nul("SDK text", bytes)?;
        Ok(Self(bytes.to_vec()))
    }

    fn from_vec(bytes: Vec<u8>) -> Result<Self, Error> {
        reject_nul("SDK text", &bytes)?;
        Ok(Self(bytes))
    }

    /// Accepts bytes already truncated at NUL by the internal FFI boundary.
    pub(crate) fn from_sdk_bytes(bytes: Vec<u8>) -> Self {
        debug_assert!(!bytes.contains(&0));
        Self(bytes)
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

    fn try_from(value: &[u8]) -> Result<Self, Error> {
        Self::new(value)
    }
}

impl TryFrom<Vec<u8>> for SdkText {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Error> {
        Self::from_vec(value)
    }
}

impl TryFrom<&str> for SdkText {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Error> {
        Self::new(value.as_bytes())
    }
}

impl TryFrom<String> for SdkText {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Error> {
        Self::from_vec(value.into_bytes())
    }
}

impl FromStr for SdkText {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        Self::try_from(value)
    }
}

/// A serial number represented as at most 16 non-NUL bytes.
#[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SerialNumber(Vec<u8>);

impl SerialNumber {
    pub const MAX_LEN: usize = 16;

    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        let bytes = bytes.as_ref();
        validate_serial(bytes)?;
        Ok(Self(bytes.to_vec()))
    }

    fn from_vec(bytes: Vec<u8>) -> Result<Self, Error> {
        validate_serial(&bytes)?;
        Ok(Self(bytes))
    }

    /// Accepts a serial copied from the SDK's fixed 16-byte field.
    #[allow(dead_code)]
    pub(crate) fn from_sdk_bytes(bytes: Vec<u8>) -> Self {
        debug_assert!(bytes.len() <= Self::MAX_LEN && !bytes.contains(&0));
        Self(bytes)
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

    fn try_from(value: &[u8]) -> Result<Self, Error> {
        Self::new(value)
    }
}

impl TryFrom<Vec<u8>> for SerialNumber {
    type Error = Error;

    fn try_from(value: Vec<u8>) -> Result<Self, Error> {
        Self::from_vec(value)
    }
}

impl TryFrom<&str> for SerialNumber {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Error> {
        Self::new(value.as_bytes())
    }
}

impl TryFrom<String> for SerialNumber {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Error> {
        Self::from_vec(value.into_bytes())
    }
}

impl FromStr for SerialNumber {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Error> {
        Self::try_from(value)
    }
}

fn validate_serial(bytes: &[u8]) -> Result<(), Error> {
    c_string("serial number", bytes, Some(SerialNumber::MAX_LEN)).map(drop)
}

fn reject_nul(field: &'static str, bytes: &[u8]) -> Result<(), Error> {
    if bytes.contains(&0) {
        return Err(Error::InvalidInput {
            field,
            violation: InputViolation::InteriorNul,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SdkText, SerialNumber};
    use crate::error::{Error, InputViolation};

    // 验证 SDK 文本保留非 UTF-8 字节，输入 C 字符串拒绝 interior NUL。
    #[test]
    fn text_preserves_bytes_and_rejects_nul() {
        let text = SdkText::new([0x66, 0x80, 0x6F]).unwrap();
        assert_eq!(text.as_bytes(), &[0x66, 0x80, 0x6F]);
        assert!(text.to_str().is_err());
        assert!(matches!(
            SerialNumber::new(b"a\0b"),
            Err(Error::InvalidInput {
                violation: InputViolation::InteriorNul,
                ..
            })
        ));
    }
}
