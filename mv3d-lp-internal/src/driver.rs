#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ptr::NonNull;

use crate::error::{ContractViolation, InputViolation};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DriverError {
    Status(i32),
    InvalidInput {
        field: &'static str,
        violation: InputViolation,
    },
    Contract(ContractViolation),
}

pub(crate) type DriverResult<T> = Result<T, DriverError>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Handle(NonNull<std::ffi::c_void>);

// SAFETY: Production handles are opaque values returned by successful device opens.
// Rust never dereferences them, and the crate's safe public API does not expose the raw
// value. `Handle` deliberately has no `Sync` implementation, so every `Device` inherits `!Sync`
// and calls for one handle cannot overlap through the safe API. Vendor evidence
// for moving handles between threads and operating distinct devices concurrently is summarized in
// `README.md` under "安全边界".
unsafe impl Send for Handle {}

impl Handle {
    pub(crate) fn from_ptr(pointer: *mut std::ffi::c_void) -> Option<Self> {
        NonNull::new(pointer).map(Self)
    }

    pub(crate) fn as_ptr(self) -> *mut std::ffi::c_void {
        self.0.as_ptr()
    }
}

pub(crate) fn status_result(status: i32) -> DriverResult<()> {
    if status == 0 {
        Ok(())
    } else {
        Err(DriverError::Status(status))
    }
}
