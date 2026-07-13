// Expected rustc error: E0505 (the camera is borrowed by the measurement).
use mv3d_lp::Camera;

fn close_with_active_measurement(mut camera: Camera<'_>) {
    let measurement = camera.start().unwrap();
    camera.close().unwrap();
    drop(measurement);
}

fn main() {}
