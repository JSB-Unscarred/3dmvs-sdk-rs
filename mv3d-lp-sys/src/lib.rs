//! Raw FFI bindings for the Hikrobot 3DMVS laser profiler SDK (LPSDK 1.3.3.3).
//!
//! The declarations are hand-audited against the three public LPSDK headers for Windows x86_64
//! MSVC. Deprecated profile-era interfaces are omitted.
//! Applications should normally use the safe `mv3d-lp` crate instead.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, clippy::upper_case_acronyms)]

include!("bindings.rs");

/// Compile-time size and offset checks copied from the audited headers.
#[cfg(all(windows, target_arch = "x86_64"))]
mod layout;

/// Implements `Default` as all-zero bytes, which the SDK requires for output and reserved fields.
macro_rules! zeroed_default {
    ($($ty:ty),+ $(,)?) => {
        $(
            impl Default for $ty {
                fn default() -> Self {
                    // SAFETY: the structure only contains integers, floats, raw pointers,
                    // byte arrays and unions of those, so the all-zero pattern is valid.
                    unsafe { core::mem::zeroed() }
                }
            }
        )+
    };
}

zeroed_default!(
    MV3D_LP_DEVICE_INFO,
    MV3D_LP_IP_CONFIG,
    MV3D_LP_IMAGE_DATA,
    MV3D_LP_PARAM,
    MV3D_LP_EXCEPTION_INFO,
    MV3D_LP_FILE_ACCESS,
    MV3D_LP_FILE_ACCESS_PROGRESS,
);
