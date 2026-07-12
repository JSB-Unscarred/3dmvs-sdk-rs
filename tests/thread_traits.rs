use mv3d_lp::{Camera, Sdk};

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

#[test]
fn sdk_and_camera_thread_traits_are_locked_at_compile_time() {}
