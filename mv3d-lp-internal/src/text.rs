use std::borrow::Cow;
use std::fmt;
use std::str::Utf8Error;

use crate::cstr::bounded_c_string;
use crate::error::{Error, InputViolation};

/// Generates the byte-owning accessors shared by every SDK text newtype.
///
/// 两个类型的访问面完全同形，只有构造校验不同；样板集中在这里，避免两份镜像各自演化。
macro_rules! sdk_bytes_newtype {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(Vec<u8>);

        impl $name {
            /// 借出原始字节；SDK 文本不保证是 UTF-8。
            #[must_use]
            pub fn as_bytes(&self) -> &[u8] {
                &self.0
            }

            /// 字节长度。
            #[must_use]
            pub const fn len(&self) -> usize {
                self.0.len()
            }

            /// 是否为空。
            #[must_use]
            pub const fn is_empty(&self) -> bool {
                self.0.is_empty()
            }

            /// 按 UTF-8 解析。
            ///
            /// # Errors
            ///
            /// 字节不是合法 UTF-8 时返回 [`Utf8Error`]。
            pub fn to_str(&self) -> Result<&str, Utf8Error> {
                std::str::from_utf8(&self.0)
            }

            /// 按 UTF-8 解析，非法序列替换为 U+FFFD。
            #[must_use]
            pub fn to_string_lossy(&self) -> Cow<'_, str> {
                String::from_utf8_lossy(&self.0)
            }

            /// 取走底层字节。
            #[must_use]
            pub fn into_bytes(self) -> Vec<u8> {
                self.0
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter
                    .debug_struct(stringify!($name))
                    .field("bytes", &self.0)
                    .field("lossy", &self.to_string_lossy())
                    .finish()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str(&self.to_string_lossy())
            }
        }

        impl AsRef<[u8]> for $name {
            fn as_ref(&self) -> &[u8] {
                self.as_bytes()
            }
        }
    };
}

sdk_bytes_newtype! {
    /// SDK-originated text kept as owned bytes without assuming an encoding.
    SdkText
}

sdk_bytes_newtype! {
    /// A serial number represented as at most 16 non-NUL bytes.
    SerialNumber
}

impl SdkText {
    /// Accepts any byte source; interior NUL is rejected so the value stays usable as `[IN]` text.
    ///
    /// # Errors
    ///
    /// 字节含有 NUL 时返回 [`Error::InvalidInput`]，因为它无法作为 C 字符串传入。
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        let bytes = bytes.as_ref();
        if bytes.contains(&0) {
            return Err(Error::InvalidInput {
                field: "SDK text",
                violation: InputViolation::InteriorNul,
            });
        }
        Ok(Self(bytes.to_vec()))
    }

    /// Accepts bytes already truncated at NUL by the internal FFI boundary.
    pub(crate) fn from_sdk_bytes(bytes: Vec<u8>) -> Self {
        debug_assert!(!bytes.contains(&0));
        Self(bytes)
    }
}

impl SerialNumber {
    /// SDK 序列号字段的固定容量，单位字节。
    pub const MAX_LEN: usize = 16;

    /// Accepts any byte source; the SDK's fixed 16-byte field bounds the length.
    ///
    /// # Errors
    ///
    /// 字节为空、含有 NUL，或超过 [`Self::MAX_LEN`] 时返回 [`Error::InvalidInput`]。
    pub fn new(bytes: impl AsRef<[u8]>) -> Result<Self, Error> {
        let bytes = bytes.as_ref();
        bounded_c_string("serial number", bytes, Self::MAX_LEN)?;
        Ok(Self(bytes.to_vec()))
    }

    /// Accepts a serial copied from the SDK's fixed 16-byte field.
    #[allow(dead_code)]
    pub(crate) fn from_sdk_bytes(bytes: Vec<u8>) -> Self {
        debug_assert!(bytes.len() <= Self::MAX_LEN && !bytes.contains(&0));
        Self(bytes)
    }
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

    // 验证序列号长度上限来自 SDK 的固定字段宽度。
    #[test]
    fn serial_number_is_bounded_by_the_sdk_field() {
        assert!(SerialNumber::new([b'A'; SerialNumber::MAX_LEN]).is_ok());
        assert!(matches!(
            SerialNumber::new([b'A'; SerialNumber::MAX_LEN + 1]),
            Err(Error::InvalidInput {
                violation: InputViolation::TooLong { .. },
                ..
            })
        ));
    }
}
