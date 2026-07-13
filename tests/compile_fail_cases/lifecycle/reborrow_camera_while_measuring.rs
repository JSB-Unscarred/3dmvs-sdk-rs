// Expected rustc error: E0499 (the measurement holds an exclusive borrow).
use mv3d_lp::Camera;

fn use_camera_with_active_measurement(camera: &mut Camera<'_>) {
    let measurement = camera.start().unwrap();
    camera.clear_buffer().unwrap();
    drop(measurement);
}

fn main() {}
