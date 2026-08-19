use mv3d_lp::{Device, DeviceInfo, ImageProcessor, Sdk};

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

// 验证共享 Runtime/processor 与独占 Device 的线程契约。
#[test]
fn public_runtime_types_follow_the_thread_contract() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Sdk>();
    assert_send_sync::<DeviceInfo>();
    assert_send_sync::<ImageProcessor>();
    assert_send::<Device>();
}
