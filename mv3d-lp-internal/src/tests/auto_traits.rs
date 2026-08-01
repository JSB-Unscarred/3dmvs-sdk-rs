use crate::driver::Handle;
use crate::runtime::RuntimeInner;
use crate::{
    CallbackMeasurement, Device, DeviceRecord, ExceptionRecord, FileTransfer,
    FileTransferStartError, FrameRecord, IpConfiguration, Measurement, ParameterRecord,
    ParameterValueRecord, Runtime,
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

#[test]
fn internal_resource_guards_remain_thread_bound() {}

#[test]
fn opaque_handle_is_send_but_not_sync() {
    fn assert_send<T: Send>() {}

    assert_send::<Handle>();
}

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

#[test]
fn borrowed_runtime_inner_is_sync() {
    fn assert_sync<T: Sync>() {}

    assert_sync::<RuntimeInner>();
}
