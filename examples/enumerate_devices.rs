//! 列出在线的激光轮廓传感器。

use mv3d_lp::Sdk;

fn main() -> mv3d_lp::Result<()> {
    println!("LPSDK {}", Sdk::version().to_string_lossy());

    let sdk = Sdk::initialize()?;
    for device in sdk.devices()? {
        println!(
            "{:<24} SN {:<16} {:?} {:?}",
            device.model_name().to_string_lossy(),
            device.serial_number().to_string_lossy(),
            device.current_ip(),
            device.ip_config_mode(),
        );
    }
    Ok(())
}
