use crate::bindings::{ImageType_Depth, ImageType_Mono8, MV3D_LP_E_DEVICE_OFFLINE};
use crate::driver::DriverError;
use crate::error::ContractViolation;
use crate::ffi::{image_from_test_buffers, zeroed_image};

// 验证失败状态优先于未可信的 SDK 输出。
#[test]
fn failing_status_is_returned_before_output_is_read() {
    assert_eq!(
        convert(MV3D_LP_E_DEVICE_OFFLINE, zeroed_image(), None, None, None),
        Err(DriverError::Status(MV3D_LP_E_DEVICE_OFFLINE))
    );
}

// 验证 pointer/length 与已知像素布局在复制前校验。
#[test]
fn image_pointer_and_lengths_are_validated_before_copying() {
    let image = base_image(ImageType_Mono8, 2, 2, 4);
    assert_eq!(
        convert(0, image, None, None, None),
        Err(DriverError::Contract(
            ContractViolation::NullPointerWithLength {
                field: "data",
                length: 4,
            }
        ))
    );

    let short_depth = base_image(ImageType_Depth, 2, 2, 7);
    assert!(matches!(
        convert(0, short_depth, Some(&[0; 8]), None, None),
        Err(DriverError::Contract(ContractViolation::LengthMismatch {
            field: "data",
            expected: 8,
            actual: 7,
        }))
    ));
}

// 验证主图、强度和 exposure 都复制为独立 owned storage。
#[test]
fn converted_frame_does_not_alias_sdk_buffers() {
    let mut data = [1, 2, 3, 4];
    let mut intensity = [5, 6, 7, 8];
    let mut exposure = [1_000_i64, 2_000];
    let mut image = base_image(ImageType_Mono8, 2, 2, 4);
    image.nIntensityDataLen = 4;
    image.nFrameNum = 42;
    image.bValid = -1;

    let frame = convert(0, image, Some(&data), Some(&intensity), Some(&exposure)).unwrap();
    data.fill(0);
    intensity.fill(0);
    exposure.fill(0);

    assert_eq!(frame.data, [1, 2, 3, 4]);
    assert_eq!(frame.intensity_data, Some(vec![5, 6, 7, 8]));
    assert_eq!(frame.exposure_timestamps, Some(vec![1_000, 2_000]));
    assert_eq!(frame.frame_number, 42);
    assert!(frame.valid);
}

fn base_image(
    image_type: i32,
    width: u32,
    height: u32,
    data_len: u32,
) -> crate::bindings::MV3D_LP_IMAGE_DATA {
    let mut image = zeroed_image();
    image.enImageType = image_type;
    image.nWidth = width;
    image.nHeight = height;
    image.nDataLen = data_len;
    image
}

fn convert(
    status: i32,
    image: crate::bindings::MV3D_LP_IMAGE_DATA,
    data: Option<&[u8]>,
    intensity: Option<&[u8]>,
    exposure: Option<&[i64]>,
) -> crate::driver::DriverResult<crate::frame::FrameRecord> {
    image_from_test_buffers(status, image, data, intensity, exposure)
}
