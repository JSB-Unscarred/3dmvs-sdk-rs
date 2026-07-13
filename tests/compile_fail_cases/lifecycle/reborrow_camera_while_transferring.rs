// Expected rustc error: E0499 (the transfer holds an exclusive borrow).
use mv3d_lp::Camera;

fn use_camera_with_active_transfer(camera: &mut Camera<'_>) {
    let transfer = camera
        .download_file(b"device.dat", b"local.dat")
        .unwrap();
    camera.clear_buffer().unwrap();
    drop(transfer);
}

fn main() {}
