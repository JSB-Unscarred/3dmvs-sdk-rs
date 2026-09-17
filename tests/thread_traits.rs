//! 公开类型的线程约定。

use mv3d_lp::{Device, DeviceInfo, Sdk};

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

// 同一 handle 的调用必须由 owner 串行发起，也保证 GetImage 输出在复制前不被覆盖。
assert_not_impl!(Device: Sync);

// 会话与设备信息可以共享，设备可以移动到工作线程。
#[test]
fn public_types_follow_the_thread_contract() {
    fn assert_send<T: Send>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Sdk>();
    assert_send_sync::<DeviceInfo>();
    assert_send::<Device>();
}
