//! 海康威视 3D 激光轮廓传感器 SDK（LPSDK）的安全 Rust 封装。
//!
//! 原始 FFI 位于 `mv3d-lp-sys`。本 crate 用所有权与借用表达 SDK 的调用约定：
//!
//! - [`Sdk`] 初始化进程级 SDK，枚举、配置并打开设备，也提供图像处理接口；
//! - [`Device`] 独占一个 native handle，负责参数、文件传输与 exception callback；
//! - [`Device::start_grabbing`] 与 [`Device::start_grabbing_with`] 返回借用设备的采集守卫，
//!   pull 取图只存在于 [`Grabbing`] 上，守卫释放时停止采集。
//!
//! 设计取舍见 [`docs::architecture`]。
//!
//! ```no_run
//! use std::net::Ipv4Addr;
//! use std::time::Duration;
//!
//! use mv3d_lp::Sdk;
//!
//! fn main() -> mv3d_lp::Result<()> {
//!     let sdk = Sdk::new()?;
//!     let mut device = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;
//!
//!     let grabbing = device.start_grabbing()?;
//!     let image = grabbing.get_image(Some(Duration::from_secs(1)))?;
//!     println!("{}x{}，{} 字节", image.width, image.height, image.data.len());
//!     Ok(())
//! }
//! ```

use std::ffi::CStr;
use std::os::raw::c_char;

pub(crate) use mv3d_lp_sys as sys;

mod callback;
mod device;
mod device_info;
pub mod docs;
mod error;
mod grabbing;
mod image;
mod kind;
mod parameter;
mod processing;
mod sdk;

pub use callback::{DeviceException, ExceptionKind};
pub use device::{Device, FileProgress};
pub use device_info::{DeviceInfo, IpConfiguration};
pub use error::{Error, ErrorCode, Result};
pub use grabbing::{CallbackGrabbing, Grabbing};
pub use image::{Image, ImageCalibration};
pub use kind::{ImageFileFormat, ImageType, IpConfigMode};
pub use parameter::{Parameter, ParameterValue};
pub use processing::DisplayRange;
pub use sdk::Sdk;

/// 读取 SDK 定长字符数组中首个 NUL 之前的字符串。
///
/// 厂商保证这些字段以 NUL 结尾；缺少 NUL 时返回空串，避免越界读取。
fn fixed_cstr(chars: &[c_char]) -> &CStr {
    // SAFETY: `c_char` 与 `u8` 大小、对齐相同，只重新解释已初始化的字节。
    let bytes = unsafe { std::slice::from_raw_parts(chars.as_ptr().cast::<u8>(), chars.len()) };
    CStr::from_bytes_until_nul(bytes).unwrap_or_default()
}
