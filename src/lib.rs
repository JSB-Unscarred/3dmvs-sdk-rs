#![forbid(unsafe_code)]

mod callback;
mod device;
#[cfg(all(windows, feature = "display-windows"))]
mod display_windows;
mod error;
mod file_transfer;
mod frame;
mod image_processor;
mod opened_device;
mod parameter;
mod sdk;
mod text;
mod types;

pub use callback::{
    CallbackOptions, CallbackStats, CallbackWorker, CallbackWorkerExit, DeviceException,
    DeviceExceptionType,
};
pub use device::{DeviceInfo, IpConfiguration, IpConfigurationMode};
#[cfg(all(windows, feature = "display-windows"))]
pub use display_windows::DisplayRange;
pub use error::{
    ContractViolation, Error, InputViolation, Operation, Result, SdkError, StatusCode,
};
pub use file_transfer::{FileProgress, FileTransferStatus};
pub use frame::{ImageCalibration, ImageRef, ImageType, OwnedFrame, OwnedImage};
pub use image_processor::{ImageFileFormat, ImageProcessor};
pub use opened_device::{Device, DeviceState};
pub use parameter::{Parameter, ParameterKind, ParameterValue};
pub use sdk::Sdk;
pub use text::{CommandKey, ParamKey, SdkText, SerialNumber};
pub use types::{ParseSdkVersionError, SdkVersion};
