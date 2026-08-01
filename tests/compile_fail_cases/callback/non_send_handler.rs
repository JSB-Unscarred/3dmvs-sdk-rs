// Expected rustc error: E0277 (`Send` is not implemented).
use std::rc::Rc;

use mv3d_lp::{CallbackOptions, Device};

fn register_non_send_handler(device: &mut Device<'_>) {
    let marker = Rc::new(());
    let _ = device.start_with_callback(CallbackOptions::default(), move |_| {
        let _ = Rc::strong_count(&marker);
    });
}

fn main() {}
