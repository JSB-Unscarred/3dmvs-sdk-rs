// Expected rustc error: E0382 (starting the transfer consumes the device).
use mv3d_lp::Device;

fn use_device_with_active_transfer(mut device: Device<'_>) {
    let transfer = device
        .download_file(b"device.dat", b"local.dat")
        .unwrap();
    device.clear_buffer().unwrap();
    drop(transfer);
}

fn main() {}
