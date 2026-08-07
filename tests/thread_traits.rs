use std::sync::mpsc::Receiver;

use mv3d_lp::{
    CallbackOptions, CallbackStats, CallbackWorker, Device, DeviceException, DeviceExceptionType,
    DeviceInfo, FileProgress, FileTransferStatus, Frame, ImageProcessor, ImageRef, IpConfiguration,
    OwnedImage, Parameter, ParameterValue, Sdk, SdkText,
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

assert_not_impl!(Device: Sync);
assert_not_impl!(OwnedImage: Clone);
assert_not_impl!(Receiver<Frame>: Sync);

// 验证进程 token 可共享，设备保持独占访问。
#[test]
fn public_runtime_types_follow_the_thread_contract() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Sdk>();
    assert_send_sync::<ImageProcessor>();
    assert_send::<Device>();
}

// 验证 owned 数据可安全跨线程传递与共享，防止 callback 输出携带线程绑定状态。
#[test]
fn frames_can_move_and_be_shared_between_threads() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn assert_send<T: Send>() {}

    assert_send_sync::<Frame>();
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
    assert_send::<Receiver<Frame>>();
}
