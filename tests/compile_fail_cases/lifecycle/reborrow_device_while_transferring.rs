// Expected rustc error: E0499 (the transfer holds an exclusive borrow).
use mv3d_lp::Device;

fn use_device_with_active_transfer(device: &mut Device<'_>) {
    let transfer = device
        .download_file(b"device.dat", b"local.dat")
        .unwrap();
    device.clear_buffer().unwrap();
    drop(transfer);
}

fn main() {}
