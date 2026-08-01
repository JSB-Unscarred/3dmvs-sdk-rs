// Expected rustc error: E0521 (borrowed data escapes the function).
use mv3d_lp::{CallbackOptions, Device};

fn register_borrowing_handler(device: &mut Device<'_>, borrowed: &str) {
    let _ = device.start_with_callback(CallbackOptions::default(), move |_| {
        let _ = borrowed.len();
    });
}

fn main() {}
