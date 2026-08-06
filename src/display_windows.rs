use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};

use crate::image_processor::{invalid, validate_layout};
use crate::{ImageProcessor, ImageRef, ImageType, InputViolation, Operation, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum DisplayRange {
    Auto,
    Manual { minimum: i32, maximum: i32 },
}

impl ImageProcessor {
    /// Draws an SDK image into a borrowed Win32 window.
    ///
    /// The window handle is held until the synchronous SDK call returns. The
    /// caller must also obey the GUI framework's drawing and thread rules. The audited native
    /// contract assumes that the SDK neither modifies the shared image payload nor retains the
    /// image or window handle after return; this is not a separate written vendor guarantee.
    pub fn display<W>(&self, image: ImageRef<'_>, window: &W, range: DisplayRange) -> Result<()>
    where
        W: HasWindowHandle + ?Sized,
    {
        validate_display_request(image, range)?;
        validate_layout(Operation::DisplayImage, image)?;
        let borrowed = window.window_handle().map_err(|error| {
            let violation = match error {
                HandleError::NotSupported => InputViolation::WindowHandleNotSupported,
                HandleError::Unavailable => InputViolation::WindowHandleUnavailable,
                _ => InputViolation::WindowHandleUnavailable,
            };
            invalid(Operation::DisplayImage, violation)
        })?;
        let hwnd = match borrowed.as_raw() {
            RawWindowHandle::Win32(handle) => handle.hwnd,
            _ => {
                return Err(invalid(
                    Operation::DisplayImage,
                    InputViolation::NonWin32Window,
                ));
            }
        };
        let range = match range {
            DisplayRange::Auto => mv3d_lp_internal::DisplayRangeRecord::Auto,
            DisplayRange::Manual { minimum, maximum } => {
                mv3d_lp_internal::DisplayRangeRecord::Manual { minimum, maximum }
            }
        };
        self.inner
            .display_image(image.to_internal(), hwnd, range)
            .map_err(crate::Error::map_internal_error)
    }
}

fn validate_display_request(image: ImageRef<'_>, range: DisplayRange) -> Result<()> {
    if !matches!(
        image.image_type,
        ImageType::MONO8
            | ImageType::DEPTH
            | ImageType::RGB24_PACKED
            | ImageType::PROFILE
            | ImageType::PROFILE_ABC32
            | ImageType::POINT_CLOUD
    ) {
        return Err(invalid(
            Operation::DisplayImage,
            InputViolation::UnsupportedDisplayImageType {
                actual: image.image_type.bits(),
            },
        ));
    }
    if let DisplayRange::Manual { minimum, maximum } = range {
        if image.image_type == ImageType::MONO8 {
            return Err(invalid(
                Operation::DisplayImage,
                InputViolation::UnsupportedDisplayMode {
                    image_type: image.image_type.bits(),
                },
            ));
        }
        if minimum >= maximum {
            return Err(invalid(
                Operation::DisplayImage,
                InputViolation::InvalidDisplayRange { minimum, maximum },
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{DisplayRange, validate_display_request};
    use crate::{ImageCalibration, ImageRef, ImageType};

    fn image(image_type: ImageType) -> ImageRef<'static> {
        ImageRef {
            image_type,
            width: 1,
            height: 1,
            data: &[0; 12],
            intensity_data: None,
            exposure_timestamps: None,
            frame_number: 0,
            device_timestamp: 0,
            valid: true,
            calibration: ImageCalibration::default(),
        }
    }

    // 验证单色显示范围与模式约束，防止无效参数进入 Windows 显示接口。
    #[test]
    fn mono_manual_and_invalid_ranges_are_rejected() {
        assert!(
            validate_display_request(
                image(ImageType::MONO8),
                DisplayRange::Manual {
                    minimum: 0,
                    maximum: 1,
                },
            )
            .is_err()
        );
        assert!(
            validate_display_request(
                image(ImageType::DEPTH),
                DisplayRange::Manual {
                    minimum: 2,
                    maximum: 2,
                },
            )
            .is_err()
        );
        assert!(validate_display_request(image(ImageType::DEPTH), DisplayRange::Auto).is_ok());
    }

    // 验证厂商文档规定的显示类型矩阵，防止不支持的图像格式传入 SDK。
    #[test]
    fn documented_display_type_matrix_is_enforced() {
        for image_type in [
            ImageType::MONO8,
            ImageType::DEPTH,
            ImageType::RGB24_PACKED,
            ImageType::PROFILE,
            ImageType::PROFILE_ABC32,
            ImageType::POINT_CLOUD,
        ] {
            assert!(validate_display_request(image(image_type), DisplayRange::Auto).is_ok());
        }
        for image_type in [
            ImageType::UNDEFINED,
            ImageType::JPEG,
            ImageType::from_bits(0x1234_5678),
        ] {
            assert!(validate_display_request(image(image_type), DisplayRange::Auto).is_err());
        }
    }
}
