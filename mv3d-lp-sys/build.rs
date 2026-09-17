//! Build script for `mv3d_lp_sys`.
//!
//! Locates the 3DMVS import library through `MV3DLP_DEV_ENV` (defaulting to the installer path)
//! and emits link directives on `x86_64-pc-windows-msvc`. When the SDK is missing it only warns,
//! so `cargo check`, `clippy` and `doc` keep working; linking then fails as expected.

use std::env;
use std::path::PathBuf;

const DEFAULT_DEVELOPMENT_ROOT: &str = r"C:\Program Files (x86)\3DMVS\Development";

fn main() {
    println!("cargo::rerun-if-changed=build.rs");
    println!("cargo::rerun-if-env-changed=MV3DLP_DEV_ENV");

    if env::var("TARGET").as_deref() != Ok("x86_64-pc-windows-msvc") {
        return;
    }

    let root = env::var_os("MV3DLP_DEV_ENV")
        .filter(|value| !value.is_empty())
        .map_or_else(|| PathBuf::from(DEFAULT_DEVELOPMENT_ROOT), PathBuf::from);
    let library_dir = root.join("Libraries").join("win64");
    if !library_dir.join("Mv3dLp.lib").is_file() {
        println!(
            "cargo::warning=Mv3dLp.lib was not found in {}; `cargo check` still works, but linking \
             requires 3DMVS. Set MV3DLP_DEV_ENV to the SDK Development directory.",
            library_dir.display()
        );
        return;
    }

    println!("cargo::rustc-link-search=native={}", library_dir.display());
    println!("cargo::rustc-link-lib=dylib=Mv3dLp");
}
