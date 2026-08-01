use std::sync::mpsc::Receiver;

use mv3d_lp::{
    CallbackMeasurement, CallbackOptions, CallbackStats, CallbackWorker, Device, DeviceException,
    DeviceExceptionType, DeviceInfo, FileProgress, FileTransfer, FileTransferStartError,
    ImageProcessor, ImageRef, IpConfiguration, Measurement, OwnedFrame, OwnedImage, Parameter,
    ParameterValue, Sdk, SdkText,
};

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
assert_not_impl!(Device<'static>: Send);
assert_not_impl!(Device<'static>: Sync);
assert_not_impl!(Measurement<'static>: Send);
assert_not_impl!(Measurement<'static>: Sync);
assert_not_impl!(CallbackMeasurement<'static>: Send);
assert_not_impl!(CallbackMeasurement<'static>: Sync);
assert_not_impl!(FileTransfer<'static>: Send);
assert_not_impl!(FileTransfer<'static>: Sync);
assert_not_impl!(FileTransferStartError<'static>: Send);
assert_not_impl!(FileTransferStartError<'static>: Sync);
assert_not_impl!(ImageProcessor<'static>: Send);
assert_not_impl!(ImageProcessor<'static>: Sync);
assert_not_impl!(OwnedFrame: Clone);
assert_not_impl!(OwnedImage: Clone);
assert_not_impl!(Receiver<OwnedFrame>: Sync);

#[test]
fn sdk_and_device_thread_traits_are_locked_at_compile_time() {}

#[test]
fn owned_frames_can_move_and_be_shared_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send<T: Send>() {}

    assert_send_sync::<OwnedFrame>();
    assert_send_sync::<OwnedImage>();
    assert_send_sync::<DeviceInfo>();
    assert_send_sync::<DeviceException>();
    assert_send_sync::<DeviceExceptionType>();
    assert_send_sync::<FileProgress>();
    assert_send_sync::<ImageRef<'static>>();
    assert_send_sync::<IpConfiguration>();
    assert_send_sync::<Parameter>();
    assert_send_sync::<ParameterValue>();
    assert_send_sync::<SdkText>();
    assert_send_sync::<CallbackOptions>();
    assert_send_sync::<CallbackStats>();
    assert_send::<CallbackWorker>();
    assert_send::<Receiver<OwnedFrame>>();
}
