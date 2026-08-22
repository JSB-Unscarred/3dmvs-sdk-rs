use std::ffi::CString;

use crate::error::{Error, InputViolation};

/// Builds a C string for one native `[IN]` argument, rejecting only interior NUL.
///
/// Node name 与文件名由 SDK 自行判定合法性，这里只保证能构造合法的 C 字符串。
pub(crate) fn c_string(field: &'static str, bytes: &[u8]) -> Result<CString, Error> {
    CString::new(bytes).map_err(|_| invalid(field, InputViolation::InteriorNul))
}

/// Additionally rejects an empty value for arguments the SDK never accepts empty.
pub(crate) fn non_empty_c_string(field: &'static str, bytes: &[u8]) -> Result<CString, Error> {
    if bytes.is_empty() {
        return Err(invalid(field, InputViolation::Empty));
    }
    c_string(field, bytes)
}

/// Additionally bounds the length for arguments copied into a fixed-width SDK field.
pub(crate) fn bounded_c_string(
    field: &'static str,
    bytes: &[u8],
    max: usize,
) -> Result<CString, Error> {
    if bytes.len() > max {
        return Err(invalid(
            field,
            InputViolation::TooLong {
                max,
                actual: bytes.len(),
            },
        ));
    }
    non_empty_c_string(field, bytes)
}

fn invalid(field: &'static str, violation: InputViolation) -> Error {
    Error::InvalidInput { field, violation }
}
