use std::sync::mpsc::Receiver;

use mv3d_lp::{
    CallbackOptions, CallbackStats, CallbackWorker, Device, DeviceException, DeviceExceptionType,
    DeviceInfo, FileProgress, FileTransferStatus, ImageProcessor, ImageRef, IpConfiguration,
    OwnedFrame, OwnedImage, Parameter, ParameterValue, Sdk, SdkText,
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
assert_not_impl!(Device<'static>: Sync);
assert_not_impl!(ImageProcessor<'static>: Send);
assert_not_impl!(ImageProcessor<'static>: Sync);
assert_not_impl!(OwnedFrame: Clone);
assert_not_impl!(OwnedImage: Clone);
assert_not_impl!(Receiver<OwnedFrame>: Sync);

// 验证设备可转移线程且保持独占访问，防止跨线程并发调用同一 handle。
#[test]
fn public_device_is_send_but_not_sync() {
    fn assert_send<T: Send>() {}

    assert_send::<Device<'static>>();
}

// 验证 owned 数据可安全跨线程传递与共享，防止 callback 输出携带线程绑定状态。
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
    assert_send_sync::<FileTransferStatus>();
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
