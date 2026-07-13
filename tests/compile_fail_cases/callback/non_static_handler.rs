// Expected rustc error: E0521 (borrowed data escapes the function).
use mv3d_lp::{CallbackOptions, Camera};

fn register_borrowing_handler(camera: &mut Camera<'_>, borrowed: &str) {
    let _ = camera.start_with_callback(CallbackOptions::default(), move |_| {
        let _ = borrowed.len();
    });
}

fn main() {}
