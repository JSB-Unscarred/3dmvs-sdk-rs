//! 海康机器人（Hikrobot）3D 激光轮廓传感器 SDK（LPSDK）的安全 Rust 封装。
//!
//! 原始 FFI 位于 `mv3d-lp-sys`。本 crate 用所有权与借用表达 SDK 的调用约定：
//!
//! - [`Sdk`] 初始化进程级 SDK，枚举、配置并打开设备，也提供图像处理接口；
//! - [`Device`] 独占一个 native handle，负责参数、文件传输与 exception callback；
//! - [`Device::start_grabbing`] 与 [`Device::start_grabbing_with`] 返回借用设备的取流守卫，
//!   pull 取图只存在于 [`Grabbing`] 上，守卫释放时停止取流。
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
//!     println!("{}x{}, {} bytes", image.width, image.height, image.data.len());
//!     Ok(())
//! }
//! ```

use std::ffi::{CStr, CString, c_char};
use std::net::Ipv4Addr;
use std::slice;

/// 原始 FFI 绑定（`mv3d-lp-sys`），与本 crate 同版本发布；配合 [`Device::as_raw_handle`]
/// 调用尚未封装的 SDK 接口。
pub use mv3d_lp_sys as sys;

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

pub use callback::{ExceptionInfo, ExceptionKind};
pub use device::Device;
pub use device_info::{DeviceInfo, IpConfig};
pub use error::{Error, ErrorCode, Result};
pub use grabbing::{CallbackGrabbing, Grabbing};
pub use image::{Image, ImageCalibration};
pub use kind::{ImageFileFormat, ImageType, IpConfigMode};
pub use parameter::{Parameter, ParameterValue};
pub use processing::DisplayRange;
pub use sdk::Sdk;

/// 读取 SDK 定长字符数组中首个 NUL 之前的字符串。
///
/// 这些字段是 C 字符串，厂商示例直接以 `%s` 读取，写入方保证以 NUL 结尾；缺少 NUL 属于违约数据，
/// 此时返回空串而不越界读取。
fn fixed_cstr(chars: &[c_char]) -> &CStr {
    CStr::from_bytes_until_nul(char_bytes(chars)).unwrap_or_default()
}

/// 复制 SDK 定长字符数组中的字符串：截到首个 NUL，字段写满、没有 NUL 时取整个字段，不丢数据。
fn fixed_cstring(chars: &[c_char]) -> CString {
    let bytes = char_bytes(chars);
    let len = bytes
        .iter()
        .position(|&byte| byte == 0)
        .unwrap_or(bytes.len());
    // 截到首个 NUL 后不含内部 NUL，CString::new 不会失败。
    CString::new(&bytes[..len]).unwrap_or_default()
}

/// 把 SDK 的 `c_char` 数组按字节读取。
fn char_bytes(chars: &[c_char]) -> &[u8] {
    // SAFETY: `c_char` 与 `u8` 大小、对齐相同，只重新解释已初始化的字节。
    unsafe { slice::from_raw_parts(chars.as_ptr().cast::<u8>(), chars.len()) }
}

/// 把 IPv4 地址写成 SDK 字段中的点分十进制文本。
///
/// 点分十进制最长 15 字节，16 字节字段必定能容纳文本与结尾 NUL。
fn write_ipv4(field: &mut [c_char; 16], ip: Ipv4Addr) {
    for (target, byte) in field.iter_mut().zip(ip.to_string().bytes()) {
        *target = byte.cast_signed();
    }
}

#[cfg(test)]
mod tests {
    use super::{fixed_cstr, fixed_cstring};

    // 拥有型字符串无损：字段写满、没有 NUL 时取整个字段；借用型按厂商约定截到 NUL，违约时为空串。
    #[test]
    fn fixed_strings_follow_the_nul_policy() {
        let full = [b'a'.cast_signed(); 4];
        assert_eq!(fixed_cstring(&full).as_bytes(), b"aaaa");
        assert_eq!(fixed_cstr(&full), c"");

        let terminated = [b'a'.cast_signed(), 0, b'b'.cast_signed(), 0];
        assert_eq!(fixed_cstring(&terminated).as_bytes(), b"a");
        assert_eq!(fixed_cstr(&terminated), c"a");
    }
}
