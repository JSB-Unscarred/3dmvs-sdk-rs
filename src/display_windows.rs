use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};

use crate::{Error, ImageProcessor, ImageRef, InputViolation, Operation, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DisplayRange {
    Auto,
    Manual { minimum: i32, maximum: i32 },
}

impl ImageProcessor {
    /// Draws an SDK image into a borrowed Win32 window.
    ///
    /// The window handle is held until the synchronous SDK call returns. The caller must also obey
    /// the GUI framework's drawing and thread rules. The internal FFI boundary validates the image
    /// descriptor; the display range is passed through for the SDK to interpret. The audited native
    /// contract assumes that the SDK neither modifies the shared image payload nor retains the image
    /// or window handle after return; this is not a separate written vendor guarantee.
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
        let range = match range {
            DisplayRange::Auto => mv3d_lp_internal::DisplayRangeRecord::Auto,
            DisplayRange::Manual { minimum, maximum } => {
                mv3d_lp_internal::DisplayRangeRecord::Manual { minimum, maximum }
            }
        };
        self.inner
            .display_image(image.to_internal(), hwnd, range)
            .map_err(Error::map_internal_error)
    }
}

/// Maps window-handle extraction failures to the display operation.
fn invalid_window(violation: InputViolation) -> Error {
    Error::InvalidInput {
        field: Operation::DisplayImage.sdk_name(),
        violation,
    }
}
