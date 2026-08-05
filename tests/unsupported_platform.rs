#[cfg(not(feature = "native"))]
// 验证禁用 native backend 时返回稳定错误，防止测试环境意外加载 SDK。
#[test]
fn disabled_native_backend_returns_a_stable_error_without_loading_the_sdk() {
    assert!(matches!(
        mv3d_lp::Sdk::initialize(),
        Err(mv3d_lp::Error::UnsupportedPlatform)
    ));
}
