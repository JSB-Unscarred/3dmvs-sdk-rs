#![deny(unsafe_op_in_unsafe_fn)]
#![deny(clippy::undocumented_unsafe_blocks)]
#![deny(improper_ctypes, improper_ctypes_definitions)]

#[cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]
mod abi;
mod bindings;
mod callback;
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

pub use callback::{
    CallbackDelivery, CallbackStatsRecord, ExceptionCallbackSink, ExceptionRecord,
    FrameCallbackSink,
};
pub use device::{DeviceRecord, IpConfiguration};
#[cfg(feature = "display-windows")]
pub use display::DisplayRangeRecord;
pub use error::{ContractViolation, Error, InvalidInput, Operation};
pub use file_transfer::{FileProgress, FileTransferDirection, FileTransferStatus};
pub use frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
pub use opened_device::{
    CallbackMeasurement, Device, DeviceCleanupError, DeviceState, FileTransfer, Measurement,
};
pub use parameter::{ParameterRecord, ParameterValueRecord};
pub use runtime::Runtime;

#[cfg(test)]
mod tests;
