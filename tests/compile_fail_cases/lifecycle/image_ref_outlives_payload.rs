// Expected rustc error: E0515 (the image cannot outlive its payload).
use mv3d_lp::{ImageCalibration, ImageRef, ImageType};

fn image_without_payload() -> ImageRef<'static> {
    let data = [0_u8; 1];
    ImageRef {
        image_type: ImageType::MONO8,
        width: 1,
        height: 1,
        data: &data,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        calibration: ImageCalibration::default(),
    }
}

fn main() {}
