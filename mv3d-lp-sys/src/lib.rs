#![doc = include_str!("../README.md")]
#![allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    reason = "names follow the vendor C headers"
)]

// layout.rs pins the audited layouts of this target, where the headers' `__stdcall` is the C ABI.
#[cfg(not(all(windows, target_arch = "x86_64", target_env = "msvc")))]
compile_error!("mv3d-lp-sys only supports x86_64-pc-windows-msvc");

include!("bindings.rs");

/// Compile-time size and offset checks copied from the audited headers.
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
    MV3D_LP_INTPARAM,
    MV3D_LP_ENUMPARAM,
    MV3D_LP_FLOATPARAM,
    MV3D_LP_STRINGPARAM,
    MV3D_LP_PARAM_INFO,
    MV3D_LP_PARAM,
    MV3D_LP_EXCEPTION_INFO,
    MV3D_LP_FILE_ACCESS,
    MV3D_LP_FILE_ACCESS_PROGRESS,
);
