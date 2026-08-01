// Expected rustc error: E0499 (the measurement holds an exclusive borrow).
use mv3d_lp::Device;

fn use_device_with_active_measurement(device: &mut Device<'_>) {
    let measurement = device.start().unwrap();
    device.clear_buffer().unwrap();
    drop(measurement);
}

fn main() {}
