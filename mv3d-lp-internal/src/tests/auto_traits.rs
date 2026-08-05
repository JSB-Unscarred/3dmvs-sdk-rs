use crate::driver::Handle;
use crate::runtime::RuntimeInner;
use crate::{
    CallbackMeasurement, Device, DeviceRecord, ExceptionRecord, FileTransfer, FrameRecord,
    IpConfiguration, Measurement, ParameterRecord, ParameterValueRecord, Runtime,
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

assert_not_impl!(Runtime: Send);
assert_not_impl!(Runtime: Sync);
assert_not_impl!(Handle: Sync);
assert_not_impl!(Device<'static>: Sync);
assert_not_impl!(Measurement<'static>: Sync);
assert_not_impl!(CallbackMeasurement<'static>: Sync);
assert_not_impl!(FileTransfer<'static>: Sync);

// 验证内部设备与活动 guard 可跨线程转移，防止无依据地限制独占所有权 handoff。
#[test]
fn internal_device_ownership_can_move_between_threads() {
    fn assert_send<T: Send>() {}

    assert_send::<Device<'static>>();
    assert_send::<Measurement<'static>>();
    assert_send::<CallbackMeasurement<'static>>();
    assert_send::<FileTransfer<'static>>();
}

// 验证 opaque handle 可转移且保持非共享，防止同一 native handle 被并发调用。
#[test]
fn opaque_handle_is_send_but_not_sync() {
    fn assert_send<T: Send>() {}

    assert_send::<Handle>();
}

// 验证 safe driver 边界记录可跨线程共享，防止 owned FFI 数据携带线程绑定引用。
#[test]
fn records_crossing_the_safe_driver_boundary_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<DeviceRecord>();
    assert_send_sync::<ExceptionRecord>();
    assert_send_sync::<FrameRecord>();
    assert_send_sync::<IpConfiguration>();
    assert_send_sync::<ParameterRecord>();
    assert_send_sync::<ParameterValueRecord>();
}

// 验证借用的 RuntimeInner 可共享，支持不同设备并行调用同一 driver。
#[test]
fn borrowed_runtime_inner_is_sync() {
    fn assert_sync<T: Sync>() {}

    assert_sync::<RuntimeInner>();
}
