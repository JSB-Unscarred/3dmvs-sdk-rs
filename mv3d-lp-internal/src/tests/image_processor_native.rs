use crate::driver::{Driver, DriverError};
use crate::error::ContractViolation;
use crate::ffi::NativeDriver;
use crate::frame::{ImageFileFormatRecord, ImageInput, ImageTypeRecord};

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
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue {
                field: "depth input",
            }
        ))
    ));
    assert!(matches!(
        driver.map_depth_to_point_cloud_round(&[]),
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue {
                field: "image count",
            }
        ))
    ));
    assert!(matches!(
        driver.convert_image(
            input,
            ImageTypeRecord::from_raw(crate::bindings::ImageType_Jpeg),
        ),
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue {
                field: "image conversion pair",
            }
        ))
    ));
    let file_name = c"image.ply";
    assert!(matches!(
        driver.save_image(input, ImageFileFormatRecord::Ply, file_name),
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue {
                field: "image file format pair",
            }
        ))
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
        Err(DriverError::Contract(ContractViolation::LengthMismatch {
            field: "data",
            expected: 1,
            actual: 2,
        }))
    ));

    let depth = [0_u8; 2];
    input.image_type = ImageTypeRecord::from_raw(crate::bindings::ImageType_Depth);
    input.data = &depth;
    input.x_scale = f32::NAN;
    assert!(matches!(
        driver.map_depth_to_point_cloud(input),
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue {
                field: "calibration scale",
            }
        ))
    ));
}
