use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};

use crate::{DisplayRange, Error, ImageRef, InputViolation, Result, Sdk};

impl Sdk {
    /// Draws an SDK image into a borrowed Win32 window.
    ///
    /// # Errors
    ///
    /// 窗口拿不到 Win32 `HWND` 时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)。
    /// 参数不满足 SDK 约束时返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，此时不会发起 native 调用。
    /// SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)；未链接 SDK 的 target 上返回 [`Error::UnsupportedPlatform`](crate::Error::UnsupportedPlatform)。
    pub fn display<W>(&self, image: ImageRef<'_>, window: &W, range: DisplayRange) -> Result<()>
    where
        W: HasWindowHandle + ?Sized,
    {
        let borrowed = window.window_handle().map_err(|error| {
            let violation = match error {
                HandleError::NotSupported => InputViolation::WindowHandleNotSupported,
                // HandleError 是 non_exhaustive；Unavailable 与未知变体同样归为拿不到句柄。
                _ => InputViolation::WindowHandleUnavailable,
            };
            invalid_window(violation)
        })?;
        let hwnd = match borrowed.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd,
            _ => return Err(invalid_window(InputViolation::NonWin32Window)),
        };
        self.inner.display_image(image, hwnd, range)
    }
}

const fn invalid_window(violation: InputViolation) -> Error {
    Error::InvalidInput {
        field: "window",
        violation,
    }
}
