//! 海康威视 3D 激光轮廓仪 3DMVS LPSDK 的安全 Rust 封装。
//!
//! [`Sdk`] 是进程级 session 的一次性入口，[`Device`] 持有一台已打开的设备。
//! native 支持只在 `x86_64-pc-windows-msvc` 且启用 `native` 特性时链接；其他
//! target 上全部接口返回 [`Error::UnsupportedPlatform`]，便于跨平台编译与检查。

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(all(windows, feature = "display-windows"))]
mod display_windows;
mod error;
mod opened_device;
mod sdk;

pub use error::{
    ContractViolation, Error, InputViolation, Operation, Result, SdkError, StatusCode,
};
#[cfg(all(windows, feature = "display-windows"))]
pub use mv3d_lp_internal::DisplayRange;
pub use mv3d_lp_internal::{
    DeviceException, DeviceExceptionType, DeviceInfo, FileProgress, Image, ImageCalibration,
    ImageFileFormat, ImageRef, ImageType, IpConfiguration, IpConfigurationMode, Parameter,
    ParameterValue, SdkText, SerialNumber,
};
pub use opened_device::Device;
pub use sdk::Sdk;
