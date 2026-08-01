const BUILD_SCRIPT: &str = include_str!("../../build.rs");

#[test]
fn native_build_requires_only_the_import_library() {
    for unused_header in ["Mv3dLpApi.h", "Mv3dLpDefine.h", "Mv3dLpImgProc.h"] {
        assert!(
            !BUILD_SCRIPT.contains(unused_header),
            "build.rs must not require unused header {unused_header}"
        );
    }

    assert!(!BUILD_SCRIPT.contains("development_root.is_dir()"));
    assert!(BUILD_SCRIPT.contains("Mv3dLp.lib"));
    assert!(BUILD_SCRIPT.contains("require_file(&import_library, \"3DMVS x64 import library\")"));
    assert!(BUILD_SCRIPT.contains("cargo:rerun-if-changed={}"));
    assert!(BUILD_SCRIPT.contains("cargo:rustc-link-search=native={}"));
    assert!(BUILD_SCRIPT.contains("cargo:rustc-link-lib=dylib=Mv3dLp"));
}
