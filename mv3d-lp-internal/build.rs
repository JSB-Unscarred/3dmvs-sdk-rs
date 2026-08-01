use std::env;
use std::path::{Path, PathBuf};

const SUPPORTED_TARGET: &str = "x86_64-pc-windows-msvc";
const DEFAULT_DEVELOPMENT_ROOT: &str = r"C:\Program Files (x86)\3DMVS\Development";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=MV3DLP_DEV_ENV");

    if env::var_os("CARGO_FEATURE_NATIVE").is_none() {
        return;
    }

    let target = env::var("TARGET").unwrap_or_else(|_| String::from("<unknown>"));
    if target != SUPPORTED_TARGET {
        panic!(
            "the `native` feature only supports target `{SUPPORTED_TARGET}`; got `{target}`. \
             Disable the `native` feature when checking or documenting another target"
        );
    }

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

    println!("cargo:rustc-link-search=native={}", library_dir.display());
    println!("cargo:rustc-link-lib=dylib=Mv3dLp");
}

fn require_file(path: &Path, description: &str) {
    if !path.is_file() {
        panic!("missing {description}: {}", path.display());
    }

    println!("cargo:rerun-if-changed={}", path.display());
}
