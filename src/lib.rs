//! 海康威视 3D 激光轮廓仪 3DMVS LPSDK 的安全 Rust 封装。
//!
//! [`Sdk`] 是进程级 session 的一次性入口，[`Device`] 持有一台已打开的设备。
//! native 支持只在 `x86_64-pc-windows-msvc` 且启用 `native` 特性时链接；其他
//! target 上全部接口返回 [`Error::UnsupportedPlatform`]，便于跨平台编译与检查。

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod opened_device;
mod sdk;

pub use mv3d_lp_internal::{
    ContractViolation, DeviceException, DeviceExceptionType, DeviceInfo, Error, FileProgress,
    Image, ImageCalibration, ImageFileFormat, ImageRef, ImageType, InputViolation, IpConfiguration,
    IpConfigurationMode, Operation, Parameter, ParameterValue, SdkError, SdkText, SerialNumber,
    StatusCode,
};
#[cfg(all(windows, feature = "display-windows"))]
pub use mv3d_lp_internal::{DisplayRange, HasWindowHandle};
pub use opened_device::Device;
pub use sdk::Sdk;

/// 本 crate 全部接口共用的 `Result` 别名。
pub type Result<T> = std::result::Result<T, Error>;
