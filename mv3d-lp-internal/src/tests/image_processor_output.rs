use crate::bindings::{ImageType_Depth, ImageType_PointCloud};
use crate::driver::DriverError;
use crate::error::ContractViolation;
use crate::ffi::{processed_image_from_test_buffers, zeroed_image};
use crate::frame::ImageTypeRecord;

// 验证 round point-cloud 输出丢弃首个输入的辅助指针，防止错误继承无关 payload。
#[test]
fn round_output_discards_first_input_auxiliary_pointers() {
    let data = [0_u8; 48];
    let intensity = [7_u8; 4];
    let exposure = [11_i64];
    let mut image = zeroed_image();
    image.enImageType = ImageType_PointCloud;
    image.nWidth = 4;
    image.nHeight = 1;
    image.nDataLen = data.len() as u32;
    image.nIntensityDataLen = intensity.len() as u32;

    let output = processed_image_from_test_buffers(
        image,
        ImageTypeRecord::from_raw(ImageType_PointCloud),
        false,
        false,
        &data,
        Some(&intensity),
        Some(&exposure),
    )
    .unwrap();

    assert_eq!(output.data, data);
    assert_eq!(output.intensity_data, None);
    assert_eq!(output.exposure_timestamps, None);
}

// 验证 mosaic 输出复制自身 intensity 平面，防止误用首个输入的辅助数据。
#[test]
fn mosaic_output_copies_its_real_intensity_plane() {
    let data = [0_u8; 8];
    let intensity = [1_u8, 2, 3, 4];
    let exposure = [10_i64, 20];
    let mut image = zeroed_image();
    image.enImageType = ImageType_Depth;
    image.nWidth = 2;
    image.nHeight = 2;
    image.nDataLen = data.len() as u32;
    image.nIntensityDataLen = intensity.len() as u32;

    let output = processed_image_from_test_buffers(
        image,
        ImageTypeRecord::from_raw(ImageType_Depth),
        true,
        false,
        &data,
        Some(&intensity),
        Some(&exposure),
    )
    .unwrap();

    assert_eq!(output.intensity_data, Some(intensity.to_vec()));
    assert_eq!(output.exposure_timestamps, None);
}

// 验证处理后的未压缩输出要求精确长度，防止接受含尾部或缺失字节的 frame。
#[test]
fn processed_uncompressed_output_requires_exact_length() {
    let data = [0_u8; 9];
    let mut image = zeroed_image();
    image.enImageType = ImageType_Depth;
    image.nWidth = 2;
    image.nHeight = 2;
    image.nDataLen = data.len() as u32;

    assert!(matches!(
        processed_image_from_test_buffers(
            image,
            ImageTypeRecord::from_raw(ImageType_Depth),
            true,
            true,
            &data,
            None,
            None,
        ),
        Err(DriverError::Contract(ContractViolation::LengthMismatch {
            field: "data",
            expected: 8,
            actual: 9,
        }))
    ));
}

// 验证处理输出类型不匹配归类为 SDK 契约错误，防止将厂商异常归责于调用方。
#[test]
fn processed_output_type_mismatch_remains_a_contract_violation() {
    let data = [0_u8; 8];
    let mut image = zeroed_image();
    image.enImageType = ImageType_Depth;
    image.nWidth = 2;
    image.nHeight = 2;
    image.nDataLen = data.len() as u32;

    assert!(matches!(
        processed_image_from_test_buffers(
            image,
            ImageTypeRecord::from_raw(ImageType_PointCloud),
            true,
            true,
            &data,
            None,
            None,
        ),
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue {
                field: "output image type",
            }
        ))
    ));
}
