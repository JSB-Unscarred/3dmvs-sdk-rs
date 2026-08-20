#![forbid(unsafe_code)]

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
