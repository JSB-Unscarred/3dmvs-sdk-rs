/// 渲染到窗口时使用的深度取值范围。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRange {
    /// 由 SDK 自动确定范围。
    Auto,
    /// 由调用方指定范围。
    Manual {
        /// 范围下界。
        minimum: i32,
        /// 范围上界。
        maximum: i32,
    },
}

/// Borrows the target's Win32 `HWND` for one synchronous display call.
///
/// 提取与失败归类收在 internal 层：窗口类 `InputViolation` 的定义与产生同处一个 crate。
#[cfg(windows)]
pub fn win32_hwnd(
    window: &(impl raw_window_handle::HasWindowHandle + ?Sized),
) -> Result<std::num::NonZeroIsize, crate::error::Error> {
    use raw_window_handle::{HandleError, RawWindowHandle};

    use crate::error::InputViolation;

    let borrowed = window.window_handle().map_err(|error| {
        let violation = match error {
            HandleError::NotSupported => InputViolation::WindowHandleNotSupported,
            // HandleError 是 non_exhaustive；Unavailable 与未知变体同样归为拿不到句柄。
            _ => InputViolation::WindowHandleUnavailable,
        };
        invalid_window(violation)
    })?;
    match borrowed.as_raw() {
        RawWindowHandle::Win32(handle) => Ok(handle.hwnd),
        _ => Err(invalid_window(InputViolation::NonWin32Window)),
    }
}

#[cfg(windows)]
const fn invalid_window(violation: crate::error::InputViolation) -> crate::error::Error {
    crate::error::Error::InvalidInput {
        field: "window",
        violation,
    }
}
