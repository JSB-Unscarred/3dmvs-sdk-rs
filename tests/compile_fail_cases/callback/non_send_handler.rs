// Expected rustc error: E0277 (`Send` is not implemented).
use std::rc::Rc;

use mv3d_lp::{CallbackOptions, Camera};

fn register_non_send_handler(camera: &mut Camera<'_>) {
    let marker = Rc::new(());
    let _ = camera.start_with_callback(CallbackOptions::default(), move |_| {
        let _ = Rc::strong_count(&marker);
    });
}

fn main() {}
