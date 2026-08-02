#[cfg(feature = "display-windows")]
use crate::DisplayRangeRecord;
use crate::driver::{Driver, DriverError};
use crate::error::InvalidInput;
use crate::ffi::{NativeDriver, validate_input_byte_limit_for_test};
use crate::frame::{ImageFileFormatRecord, ImageInput, ImageTypeRecord};
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;

#[test]
fn native_boundary_rejects_wrong_types_and_counts_before_calling_sdk() {
    let mono = [0_u8; 1];
    let input = ImageInput {
        image_type: ImageTypeRecord::from_raw(crate::bindings::ImageType_Mono8),
        width: 1,
        height: 1,
        data: &mono,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };
    let driver = NativeDriver;

    assert!(matches!(
        driver.map_depth_to_point_cloud(input),
        Err(DriverError::InvalidInput(
            InvalidInput::UnexpectedImageType { expected, actual }
        )) if expected == crate::bindings::ImageType_Depth as u32
            && actual == crate::bindings::ImageType_Mono8 as u32
    ));
    assert!(matches!(
        driver.map_depth_to_point_cloud_round(&[]),
        Err(DriverError::InvalidInput(InvalidInput::ImageCount {
            minimum: 1,
            maximum: 8,
            actual: 0,
        }))
    ));
    assert!(matches!(
        driver.convert_image(
            input,
            ImageTypeRecord::from_raw(crate::bindings::ImageType_Jpeg),
        ),
        Err(DriverError::InvalidInput(
            InvalidInput::UnsupportedImageConversion { source, target }
        )) if source == crate::bindings::ImageType_Mono8 as u32
            && target == crate::bindings::ImageType_Jpeg as u32
    ));
    let file_name = c"image.ply";
    assert!(matches!(
        driver.save_image(input, ImageFileFormatRecord::Ply, file_name),
        Err(DriverError::InvalidInput(
            InvalidInput::UnsupportedImageFileFormat {
                image_type,
                file_format,
            }
        )) if image_type == crate::bindings::ImageType_Mono8 as u32
            && file_format == ImageFileFormatRecord::Ply as i32
    ));
}

#[test]
fn native_boundary_requires_tightly_packed_finite_inputs() {
    let padded_mono = [0_u8; 2];
    let mut input = ImageInput {
        image_type: ImageTypeRecord::from_raw(crate::bindings::ImageType_Mono8),
        width: 1,
        height: 1,
        data: &padded_mono,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };
    let driver = NativeDriver;

    assert!(matches!(
        driver.save_image(input, ImageFileFormatRecord::Bmp, c"image.bmp"),
        Err(DriverError::InvalidInput(
            InvalidInput::InvalidImageLayout {
                field: "data length",
            }
        ))
    ));

    let depth = [0_u8; 2];
    input.image_type = ImageTypeRecord::from_raw(crate::bindings::ImageType_Depth);
    input.data = &depth;
    input.x_scale = f32::NAN;
    assert!(matches!(
        driver.map_depth_to_point_cloud(input),
        Err(DriverError::InvalidInput(
            InvalidInput::InvalidImageLayout {
                field: "calibration scale",
            }
        ))
    ));
}

#[test]
fn native_boundary_reports_input_limits_as_invalid_input() {
    const LIMIT: usize = 512 * 1024 * 1024;

    assert_eq!(validate_input_byte_limit_for_test(LIMIT), Ok(()));
    assert_eq!(
        validate_input_byte_limit_for_test(LIMIT + 1),
        Err(DriverError::InvalidInput(InvalidInput::TooLong {
            maximum: LIMIT,
            actual: LIMIT + 1,
        }))
    );
}

#[test]
fn native_boundary_matches_public_validation_priority() {
    let depth = [0_u8; 2];
    let oversized_dimensions = ImageInput {
        image_type: ImageTypeRecord::from_raw(crate::bindings::ImageType_Depth),
        width: 70_000,
        height: 1_000,
        data: &depth,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };
    let padded_mono = [0_u8; 2];
    let invalid_mono = ImageInput {
        image_type: ImageTypeRecord::from_raw(crate::bindings::ImageType_Mono8),
        width: 1,
        height: 1,
        data: &padded_mono,
        ..oversized_dimensions
    };
    let layout_error = DriverError::InvalidInput(InvalidInput::InvalidImageLayout {
        field: "data length",
    });

    assert_eq!(
        NativeDriver
            .map_depth_to_point_cloud(oversized_dimensions)
            .unwrap_err(),
        layout_error
    );
    assert_eq!(
        NativeDriver
            .map_depth_to_point_cloud_round(&[oversized_dimensions])
            .unwrap_err(),
        layout_error
    );
    assert_eq!(
        NativeDriver
            .convert_image(
                invalid_mono,
                ImageTypeRecord::from_raw(crate::bindings::ImageType_Jpeg),
            )
            .unwrap_err(),
        layout_error
    );
    assert_eq!(
        NativeDriver
            .save_image(invalid_mono, ImageFileFormatRecord::Ply, c"image.ply")
            .unwrap_err(),
        layout_error
    );
}

#[cfg(feature = "display-windows")]
#[test]
fn native_boundary_reports_invalid_display_requests_as_input() {
    let byte = [0_u8; 1];
    let depth = [0_u8; 2];
    let mut input = ImageInput {
        image_type: ImageTypeRecord::from_raw(crate::bindings::ImageType_Jpeg),
        width: 1,
        height: 1,
        data: &byte,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };
    let window = NonZeroIsize::new(1).unwrap();

    assert_eq!(
        NativeDriver.display_image(input, window, DisplayRangeRecord::Auto),
        Err(DriverError::InvalidInput(
            InvalidInput::UnsupportedDisplayImageType {
                actual: crate::bindings::ImageType_Jpeg as u32,
            }
        ))
    );

    input.image_type = ImageTypeRecord::from_raw(crate::bindings::ImageType_Mono8);
    assert_eq!(
        NativeDriver.display_image(
            input,
            window,
            DisplayRangeRecord::Manual {
                minimum: 0,
                maximum: 1,
            },
        ),
        Err(DriverError::InvalidInput(
            InvalidInput::UnsupportedDisplayMode {
                image_type: crate::bindings::ImageType_Mono8 as u32,
            }
        ))
    );

    input.image_type = ImageTypeRecord::from_raw(crate::bindings::ImageType_Depth);
    input.data = &depth;
    assert_eq!(
        NativeDriver.display_image(
            input,
            window,
            DisplayRangeRecord::Manual {
                minimum: 10,
                maximum: 10,
            },
        ),
        Err(DriverError::InvalidInput(
            InvalidInput::InvalidDisplayRange {
                minimum: 10,
                maximum: 10,
            }
        ))
    );
}
