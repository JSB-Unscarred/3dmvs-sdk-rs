#[cfg(not(feature = "native"))]
#[test]
fn disabled_native_backend_returns_a_stable_error_without_loading_the_sdk() {
    assert!(matches!(
        mv3d_lp::Sdk::initialize(),
        Err(mv3d_lp::Error::UnsupportedPlatform)
    ));
}
