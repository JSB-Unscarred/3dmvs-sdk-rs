use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};

use super::mock_driver::{MockDriver, active_runtime};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;

fn input<'a>(data: &'a [u8]) -> ImageInput<'a> {
    ImageInput {
        image_type: ImageTypeRecord::from_bits(0x0110_00B8),
        width: 2,
        height: 2,
        data,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 7,
        device_timestamp: 9,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    }
}

fn output(image_type: u32, bytes: usize) -> FrameRecord {
    FrameRecord {
        image_type: ImageTypeRecord::from_bits(image_type),
        width: 2,
        height: 2,
        data: vec![0xA5; bytes],
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 7,
        device_timestamp: 9,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    }
}

#[test]
fn runtime_routes_all_image_processing_operations() {
    let mock = MockDriver::new();
    mock.push_map_depth_to_point_cloud(Ok(output(0x0260_00C0, 48)));
    mock.push_map_depth_to_point_cloud_round(Ok(output(0x0260_00C0, 48)));
    mock.push_convert_image(Ok(output(0x0108_0001, 4)));
    mock.push_mosaic_depth(Ok(output(0x0110_00B8, 8)));
    mock.push_save_image(Ok(()));
    let (runtime, _) = active_runtime(&mock);
    let depth = [0_u8; 8];
    let input = input(&depth);

    runtime.map_depth_to_point_cloud(input).unwrap();
    runtime.map_depth_to_point_cloud_round(&[input]).unwrap();
    runtime
        .convert_image(input, ImageTypeRecord::from_bits(0x0108_0001))
        .unwrap();
    runtime.mosaic_depth(&[input]).unwrap();
    runtime
        .save_image(input, ImageFileFormatRecord::TiffU16, b"image.tiff")
        .unwrap();

    let logs = mock.logs();
    for operation in [
        "map_depth_to_point_cloud",
        "map_depth_to_point_cloud_round",
        "convert_image",
        "mosaic_depth",
        "save_image",
    ] {
        assert!(logs.contains(&operation));
    }
}

#[cfg(feature = "display-windows")]
#[test]
fn runtime_routes_windows_display_through_the_driver() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let depth = [0_u8; 8];

    runtime
        .display_image(
            input(&depth),
            NonZeroIsize::new(1).unwrap(),
            DisplayRangeRecord::Manual {
                minimum: 1,
                maximum: 10,
            },
        )
        .unwrap();

    assert!(mock.logs().contains(&"display_image"));
    let calls = mock.display_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].window.get(), 1);
    assert_eq!(
        calls[0].range,
        DisplayRangeRecord::Manual {
            minimum: 1,
            maximum: 10,
        }
    );

    mock.push_display_image(Err(crate::driver::DriverError::Status(
        0x8006_0005_u32 as i32,
    )));
    assert!(matches!(
        runtime.display_image(
            input(&depth),
            NonZeroIsize::new(2).unwrap(),
            DisplayRangeRecord::Auto,
        ),
        Err(crate::Error::Sdk {
            operation: "MV3D_LP_DisplayImage",
            status,
        }) if status == 0x8006_0005_u32 as i32
    ));
}
