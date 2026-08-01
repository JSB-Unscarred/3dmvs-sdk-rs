// Expected rustc error: E0505 (the device is borrowed by the measurement).
use mv3d_lp::Device;

fn close_with_active_measurement(mut device: Device<'_>) {
    let measurement = device.start().unwrap();
    device.close().unwrap();
    drop(measurement);
}

fn main() {}
