use std::ffi::CString;

use crate::error::{Error, InputViolation};

/// Builds a C string for one native `[IN]` argument.
///
/// `max` also rejects empty input. Parameter keys omit `max` and only reject interior NUL.
pub(crate) fn c_string(
    field: &'static str,
    bytes: &[u8],
    max: Option<usize>,
) -> Result<CString, Error> {
    if let Some(max) = max {
        if bytes.is_empty() {
            return Err(Error::InvalidInput {
                field,
                violation: InputViolation::Empty,
            });
        }
        if bytes.len() > max {
            return Err(Error::InvalidInput {
                field,
                violation: InputViolation::TooLong {
                    max,
                    actual: bytes.len(),
                },
            });
        }
    }
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        field,
        violation: InputViolation::InteriorNul,
    })
}
