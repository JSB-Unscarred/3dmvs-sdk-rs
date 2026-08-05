use crate::bindings::{
    ImageType_Depth, ImageType_Jpeg, ImageType_Mono8, ImageType_PointCloud, ImageType_Profile,
    ImageType_Profile_ABC32, ImageType_RGB24_Packed, MV3D_LP_E_DEVICE_OFFLINE, MV3D_LP_IMAGE_DATA,
};
use crate::driver::DriverError;
use crate::error::ContractViolation;
use crate::ffi::{FrameLimits, image_from_test_buffers, zeroed_image};

// 验证 SDK 失败状态优先于输出解析，防止读取失败调用留下的未可信 descriptor。
#[test]
fn failing_status_wins_over_untrusted_output() {
    let image = zeroed_image();

    assert_eq!(
        convert(
            MV3D_LP_E_DEVICE_OFFLINE,
            image,
            None,
            None,
            None,
            FrameLimits::default(),
        ),
        Err(DriverError::Status(MV3D_LP_E_DEVICE_OFFLINE))
    );
}

// 验证非零数据长度要求有效指针，防止从空地址复制图像 payload。
#[test]
fn null_data_pointer_with_nonzero_length_is_rejected() {
    let image = base_image(ImageType_Mono8, 2, 2, 4);

    assert_eq!(
        convert_ok(image, None, None, None),
        Err(DriverError::Contract(
            ContractViolation::NullPointerWithLength {
                field: "data",
                length: 4,
            }
        ))
    );
}

// 验证非零强度长度要求有效指针，防止从空地址复制 intensity payload。
#[test]
fn null_intensity_pointer_with_nonzero_length_is_rejected() {
    let mut image = base_image(ImageType_Mono8, 2, 2, 4);
    image.nIntensityDataLen = 4;

    assert_eq!(
        convert_ok(image, Some(&[0; 4]), None, None),
        Err(DriverError::Contract(
            ContractViolation::NullPointerWithLength {
                field: "intensity data",
                length: 4,
            }
        ))
    );
}

// 验证零长度 intensity 忽略非空指针，防止对空切片附加无依据的指针对齐要求。
#[test]
fn a_nonnull_zero_length_intensity_pointer_is_ignored() {
    let image = base_image(ImageType_Mono8, 2, 2, 4);

    let frame = convert_ok(image, Some(&[1, 2, 3, 4]), Some(&[]), None).unwrap();

    assert_eq!(frame.intensity_data, None);
}

// 验证已知未压缩格式满足最小字节数，防止按尺寸读取短缓冲区。
#[test]
fn every_known_uncompressed_format_requires_its_minimum_length() {
    let formats = [
        (ImageType_Mono8, 1_usize),
        (ImageType_Depth, 2),
        (ImageType_Profile, 6),
        (ImageType_PointCloud, 12),
        (ImageType_RGB24_Packed, 3),
        (ImageType_Profile_ABC32, 12),
    ];

    for (image_type, bytes_per_pixel) in formats {
        let expected = 6 * bytes_per_pixel;
        let data = vec![0xA5; expected];
        let image = base_image(image_type, 2, 3, expected as u32);
        assert!(convert_ok(image, Some(&data), None, None).is_ok());

        let padded_data = vec![0xA5; expected + 1];
        let padded = base_image(image_type, 2, 3, (expected + 1) as u32);
        assert!(convert_ok(padded, Some(&padded_data), None, None).is_ok());

        let malformed = base_image(image_type, 2, 3, (expected - 1) as u32);
        assert!(matches!(
            convert_ok(malformed, Some(&data), None, None),
            Err(DriverError::Contract(
                ContractViolation::LengthMismatch {
                    field: "data",
                    expected: value,
                    actual,
                }
            )) if value == expected && actual == expected - 1
        ));
    }
}

// 验证 JPEG 与未知格式使用 SDK 报告长度，防止套用未定义的像素布局。
#[test]
fn jpeg_and_unknown_types_use_reported_data_length() {
    for image_type in [ImageType_Jpeg, 0x1234_5678_i32] {
        let data = [1, 2, 3, 4, 5];
        let image = base_image(image_type, 11, 7, data.len() as u32);

        let frame = convert_ok(image, Some(&data), None, None).unwrap();

        assert_eq!(frame.image_type.raw(), image_type);
        assert_eq!(frame.data, data);
    }
}

// 验证图像尺寸乘法溢出在复制前被拒绝，防止绕回后的长度校验失效。
#[test]
fn data_size_arithmetic_overflow_is_rejected_before_copying() {
    let image = base_image(ImageType_PointCloud, u32::MAX, u32::MAX, u32::MAX);

    assert!(matches!(
        convert_ok(image, Some(&[0]), None, None),
        Err(DriverError::Contract(
            ContractViolation::LengthOverflow { .. }
        ))
    ));
}

// 验证 aggregate 大小上限在读取 backing memory 前检查，防止超大 descriptor 触发访问。
#[test]
fn aggregate_limit_is_checked_before_backing_memory_is_read() {
    let image = base_image(0x1234_5678, 1, 1, 9);

    assert_eq!(
        convert(
            0,
            image,
            Some(&[0]),
            None,
            None,
            FrameLimits::with_max_frame_bytes(8),
        ),
        Err(DriverError::Contract(ContractViolation::OutputTooLarge {
            field: "frame payloads",
            limit: 8,
            actual: 9,
        }))
    );
}

// 验证 aggregate 大小恰好等于上限时可接受，防止合法边界值被拒绝。
#[test]
fn aggregate_limit_is_inclusive() {
    let data = [1, 2, 3, 4];
    let intensity = [5, 6, 7, 8];
    let exposure = [9_i64, 10];
    let mut image = base_image(ImageType_Mono8, 2, 2, data.len() as u32);
    image.nIntensityDataLen = intensity.len() as u32;

    let frame = convert(
        0,
        image,
        Some(&data),
        Some(&intensity),
        Some(&exposure),
        FrameLimits::with_max_frame_bytes(24),
    )
    .unwrap();

    assert_eq!(frame.data, data);
    assert_eq!(frame.intensity_data.as_deref(), Some(intensity.as_slice()));
    assert_eq!(
        frame.exposure_timestamps.as_deref(),
        Some(exposure.as_slice())
    );
    assert!(!frame.valid, "bValid == 0 remains an owned invalid frame");
}

// 验证 intensity 长度严格等于像素数，防止辅助平面布局与主图尺寸脱节。
#[test]
fn intensity_length_must_equal_the_pixel_count() {
    let data = [0; 4];
    let intensity = [0; 3];
    let mut image = base_image(ImageType_Mono8, 2, 2, data.len() as u32);
    image.nIntensityDataLen = intensity.len() as u32;

    assert_eq!(
        convert_ok(image, Some(&data), Some(&intensity), None),
        Err(DriverError::Contract(ContractViolation::LengthMismatch {
            field: "intensity data",
            expected: 4,
            actual: 3,
        }))
    );
}

// 验证零宽或零高图像被拒绝，防止构造无有效像素布局的 frame。
#[test]
fn zero_dimensions_are_rejected() {
    let image = base_image(ImageType_Jpeg, 0, 1, 1);

    assert_eq!(
        convert_ok(image, Some(&[0]), None, None),
        Err(DriverError::Contract(
            ContractViolation::InvalidImageValue { field: "width" }
        ))
    );
}

// 验证转换后的 frame 拥有独立数据，防止 Rust 输出继续别名 SDK 缓冲区。
#[test]
fn copied_frame_does_not_alias_sdk_buffers() {
    let mut data = [1, 2, 3, 4];
    let mut intensity = [5, 6, 7, 8];
    let mut exposure = [1_000_i64, 2_000];
    let mut image = base_image(ImageType_Mono8, 2, 2, data.len() as u32);
    image.nIntensityDataLen = intensity.len() as u32;
    image.nFrameNum = 42;
    image.nTimeStamp = 123_456;
    image.bValid = -1;
    image.fXScale = 0.1;
    image.fYScale = 0.2;
    image.fZScale = 0.3;
    image.nXOffset = -10;
    image.nYOffset = 20;
    image.nZOffset = -30;

    let frame = convert_ok(image, Some(&data), Some(&intensity), Some(&exposure)).unwrap();

    data.fill(0);
    intensity.fill(0);
    exposure.fill(0);

    assert_eq!(frame.data, [1, 2, 3, 4]);
    assert_eq!(frame.intensity_data, Some(vec![5, 6, 7, 8]));
    assert_eq!(frame.exposure_timestamps, Some(vec![1_000, 2_000]));
    assert_eq!(frame.frame_number, 42);
    assert_eq!(frame.device_timestamp, 123_456);
    assert!(frame.valid);
    assert_eq!(
        (frame.x_scale, frame.y_scale, frame.z_scale),
        (0.1, 0.2, 0.3)
    );
    assert_eq!(
        (frame.x_offset, frame.y_offset, frame.z_offset),
        (-10, 20, -30)
    );
}

fn base_image(image_type: i32, width: u32, height: u32, data_len: u32) -> MV3D_LP_IMAGE_DATA {
    let mut image = zeroed_image();
    image.enImageType = image_type;
    image.nWidth = width;
    image.nHeight = height;
    image.nDataLen = data_len;
    image
}

fn convert_ok(
    image: MV3D_LP_IMAGE_DATA,
    data: Option<&[u8]>,
    intensity_data: Option<&[u8]>,
    exposure_timestamps: Option<&[i64]>,
) -> crate::driver::DriverResult<crate::frame::FrameRecord> {
    convert(
        0,
        image,
        data,
        intensity_data,
        exposure_timestamps,
        FrameLimits::default(),
    )
}

fn convert(
    status: i32,
    image: MV3D_LP_IMAGE_DATA,
    data: Option<&[u8]>,
    intensity_data: Option<&[u8]>,
    exposure_timestamps: Option<&[i64]>,
    limits: FrameLimits,
) -> crate::driver::DriverResult<crate::frame::FrameRecord> {
    image_from_test_buffers(
        status,
        image,
        data,
        intensity_data,
        exposure_timestamps,
        limits,
    )
}
