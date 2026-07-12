use mv3d_lp::{Camera, FileTransfer, ImageProcessor, Measurement, OwnedFrame, OwnedImage, Sdk};

macro_rules! assert_not_impl {
    ($type:ty: $bound:path) => {
        const _: fn() = || {
            trait AmbiguousIfImplemented<Marker> {
                fn marker() {}
            }

            impl<T: ?Sized> AmbiguousIfImplemented<()> for T {}

            struct ImplementsBound;
            impl<T: ?Sized + $bound> AmbiguousIfImplemented<ImplementsBound> for T {}

            let _ = <$type as AmbiguousIfImplemented<_>>::marker;
        };
    };
}

assert_not_impl!(Sdk: Send);
assert_not_impl!(Sdk: Sync);
assert_not_impl!(Camera<'static>: Send);
assert_not_impl!(Camera<'static>: Sync);
assert_not_impl!(Measurement<'static>: Send);
assert_not_impl!(Measurement<'static>: Sync);
assert_not_impl!(FileTransfer<'static, 'static>: Send);
assert_not_impl!(FileTransfer<'static, 'static>: Sync);
assert_not_impl!(ImageProcessor<'static>: Send);
assert_not_impl!(ImageProcessor<'static>: Sync);
assert_not_impl!(OwnedFrame: Clone);
assert_not_impl!(OwnedImage: Clone);

#[test]
fn sdk_and_camera_thread_traits_are_locked_at_compile_time() {}

#[test]
fn owned_frames_can_move_and_be_shared_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<OwnedFrame>();
}
