//! 打开第一台设备，在 callback 中把图像交给主线程。

use std::sync::mpsc;
use std::time::Duration;

use mv3d_lp::{ExceptionKind, Sdk};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let sdk = Sdk::new()?;
    let devices = sdk.devices()?;
    let info = devices.first().ok_or("no device found")?;
    let mut device = sdk.open_by_serial(info.serial_number())?;

    device.register_exception_callback(|exception| {
        if exception.kind == ExceptionKind::Disconnected {
            eprintln!(
                "device disconnected: {}",
                exception.description.to_string_lossy()
            );
        }
    })?;

    // callback 在 SDK 线程运行：图像已复制，直接交给主线程处理。
    let (sender, receiver) = mpsc::sync_channel(4);
    let grabbing = device.start_grabbing_with(move |image| {
        let _ = sender.try_send(image);
    })?;
    for _ in 0..10 {
        let image = receiver.recv_timeout(Duration::from_secs(1))?;
        println!(
            "#{} {}x{} {} bytes",
            image.frame_number,
            image.width,
            image.height,
            image.data.len()
        );
    }
    grabbing.stop().1?;

    device.close()?;
    Ok(())
}
