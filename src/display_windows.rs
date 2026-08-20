use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};

use crate::{DisplayRange, Error, ImageRef, InputViolation, Result, Sdk};

impl Sdk {
    /// Draws an SDK image into a borrowed Win32 window.
    pub fn display<W>(&self, image: ImageRef<'_>, window: &W, range: DisplayRange) -> Result<()>
    where
        W: HasWindowHandle + ?Sized,
    {
        let borrowed = window.window_handle().map_err(|error| {
            let violation = match error {
                HandleError::NotSupported => InputViolation::WindowHandleNotSupported,
                HandleError::Unavailable => InputViolation::WindowHandleUnavailable,
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

fn invalid_window(violation: InputViolation) -> Error {
    Error::InvalidInput {
        field: "window",
        violation,
    }
}
