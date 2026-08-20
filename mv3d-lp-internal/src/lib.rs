#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(improper_ctypes, improper_ctypes_definitions)]

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
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
pub use runtime::Runtime;
pub use text::{SdkText, SerialNumber};
