#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
mod abi;
mod bindings;
mod callback;
mod camera;
mod device;
#[cfg(feature = "display-windows")]
mod display;
mod driver;
mod error;
mod ffi;
mod file_transfer;
mod frame;
mod parameter;
mod runtime;

pub use callback::{
    CallbackDelivery, CallbackStatsRecord, ExceptionCallbackSink, ExceptionRecord,
    FrameCallbackSink,
};
pub use camera::{
    CallbackMeasurement, Camera, CameraState, CleanupError, FileTransfer, Measurement,
};
pub use device::{DeviceRecord, IpConfiguration};
#[cfg(feature = "display-windows")]
pub use display::DisplayRangeRecord;
pub use error::{ContractViolation, Error, InvalidInput};
pub use file_transfer::{FileProgress, FileTransferDirection, FileTransferStatus};
pub use frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
pub use parameter::{ParameterRecord, ParameterValueRecord};
pub use runtime::Runtime;

#[cfg(test)]
mod tests;
