//! 打开第一台设备，以 pull 方式取 10 帧，并把最后一帧深度图转换为点云保存。

use std::time::Duration;

use mv3d_lp::{ImageFileFormat, ImageType, Sdk};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = Sdk::new()?;
    let devices = sdk.devices()?;
    let info = devices.first().ok_or("no device found")?;
    let mut device = sdk.open_by_serial(info.serial_number())?;

    let grabbing = device.start_grabbing()?;
    let mut last = None;
    for _ in 0..10 {
        let image = grabbing.get_image(Some(Duration::from_secs(1)))?;
        println!(
            "#{} {:?} {}x{}",
            image.frame_number, image.image_type, image.width, image.height
        );
        last = Some(image);
    }
    grabbing.stop()?;

    if let Some(image) = last.filter(|image| image.image_type == ImageType::DEPTH) {
        let cloud = sdk.depth_to_point_cloud(&image)?;
        sdk.save_image(&cloud, ImageFileFormat::Ply, c"cloud.ply")?;
    }

    device.close()?;
    Ok(())
}
