#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
mod abi;
mod bindings;
mod camera;
mod device;
mod driver;
mod error;
mod ffi;
mod frame;
mod parameter;
mod runtime;

pub use camera::{Camera, CameraState, CleanupError, Measurement};
pub use device::{DeviceRecord, IpConfiguration};
pub use error::{ContractViolation, Error, InvalidInput};
pub use frame::{FrameRecord, ImageTypeRecord};
pub use parameter::{ParameterRecord, ParameterValueRecord};
pub use runtime::Runtime;

#[cfg(test)]
mod tests;
