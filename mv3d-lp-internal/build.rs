use std::env;
use std::path::{Path, PathBuf};

const SUPPORTED_TARGET: &str = "x86_64-pc-windows-msvc";
const DEFAULT_DEVELOPMENT_ROOT: &str = r"C:\Program Files (x86)\3DMVS\Development";

/// Emits the two combined cfg aliases used across the crate and links the vendor import library.
///
/// `sdk_target` marks the audited Windows x86_64 MSVC ABI; `native_sdk` additionally requires the
/// `native` feature. Publishing them here keeps the four-condition predicate in one place.
fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=MV3DLP_DEV_ENV");
    // Custom cfg values must be declared or `unexpected_cfgs` fires under `-D warnings`.
    println!("cargo::rustc-check-cfg=cfg(sdk_target)");
    println!("cargo::rustc-check-cfg=cfg(native_sdk)");

    let target = env::var("TARGET").unwrap_or_else(|_| String::from("<unknown>"));
    let sdk_target = target == SUPPORTED_TARGET;
    if sdk_target {
        println!("cargo::rustc-cfg=sdk_target");
    }

    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }

    if !sdk_target {
        panic!(
            "the `native` feature only supports target `{SUPPORTED_TARGET}`; got `{target}`. \
             Disable the `native` feature when checking or documenting another target"
        );
    }
    println!("cargo::rustc-cfg=native_sdk");

    let development_root = env::var_os("MV3DLP_DEV_ENV")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_DEVELOPMENT_ROOT));

    configure_native_link(&development_root);
}

fn configure_native_link(development_root: &Path) {
    let library_dir = development_root.join("Libraries").join("win64");
    let import_library = library_dir.join("Mv3dLp.lib");
    require_file(&import_library, "3DMVS x64 import library");

    println!("cargo::rustc-link-search=native={}", library_dir.display());
    println!("cargo::rustc-link-lib=dylib=Mv3dLp");
}

fn require_file(path: &Path, description: &str) {
    if !path.is_file() {
        panic!("missing {description}: {}", path.display());
    }

    println!("cargo::rerun-if-changed={}", path.display());
}
