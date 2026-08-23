//! 海康威视 3D 激光轮廓仪 LPSDK 的私有 FFI 边界。
//!
//! 这里承担全部 native 调用、指针与长度校验以及数据拷贝；对外 API 由 `mv3d-lp`
//! re-export。native 调用只在 `native_sdk` 成立时编译，其他 target 返回
//! [`Error::UnsupportedPlatform`]。

#![deny(missing_docs)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(improper_ctypes, improper_ctypes_definitions)]

#[cfg(sdk_target)]
mod abi;
mod bindings;
mod bits;
mod callback;
mod cstr;
mod device;
#[cfg(feature = "display-windows")]
mod display;
mod driver;
mod error;
mod ffi;
mod file_transfer;
mod frame;
mod opened_device;
mod parameter;
mod runtime;
mod text;

pub use callback::{DeviceException, DeviceExceptionType, ExceptionCallback, ImageCallback};
pub use device::{DeviceInfo, IpConfiguration, IpConfigurationMode};
#[cfg(feature = "display-windows")]
pub use display::DisplayRange;
pub use error::{ContractViolation, Error, InputViolation, Operation, SdkError, StatusCode};
pub use file_transfer::FileProgress;
pub use frame::{Image, ImageCalibration, ImageFileFormat, ImageRef, ImageType};
pub use opened_device::Device;
pub use parameter::{Parameter, ParameterValue};
#[cfg(all(windows, feature = "display-windows"))]
pub use raw_window_handle::HasWindowHandle;
pub use runtime::Runtime;
pub use text::{SdkText, SerialNumber};
