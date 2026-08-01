// Expected rustc error: E0277 (`FileTransfer` is not `Send`).
use mv3d_lp::FileTransfer;

fn move_transfer_to_scoped_thread(transfer: FileTransfer<'_>) {
    std::thread::scope(|scope| {
        scope.spawn(move || drop(transfer));
    });
}

fn main() {}
