use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

struct CompileFailCase {
    bin: &'static str,
    source: &'static str,
    error_code: &'static str,
}

const CASES: &[CompileFailCase] = &[
    CompileFailCase {
        bin: "non_send_handler",
        source: include_str!("compile_fail_cases/callback/non_send_handler.rs"),
        error_code: "E0277",
    },
    CompileFailCase {
        bin: "non_static_handler",
        source: include_str!("compile_fail_cases/callback/non_static_handler.rs"),
        error_code: "E0521",
    },
    CompileFailCase {
        bin: "device_outlives_sdk",
        source: include_str!("compile_fail_cases/lifecycle/device_outlives_sdk.rs"),
        error_code: "E0515",
    },
    CompileFailCase {
        bin: "close_while_measuring",
        source: include_str!("compile_fail_cases/lifecycle/close_while_measuring.rs"),
        error_code: "E0505",
    },
    CompileFailCase {
        bin: "image_ref_outlives_payload",
        source: include_str!("compile_fail_cases/lifecycle/image_ref_outlives_payload.rs"),
        error_code: "E0515",
    },
    CompileFailCase {
        bin: "reborrow_device_while_measuring",
        source: include_str!("compile_fail_cases/lifecycle/reborrow_device_while_measuring.rs"),
        error_code: "E0499",
    },
    CompileFailCase {
        bin: "reborrow_device_while_transferring",
        source: include_str!("compile_fail_cases/lifecycle/reborrow_device_while_transferring.rs"),
        error_code: "E0499",
    },
    CompileFailCase {
        bin: "shutdown_while_device_borrowed",
        source: include_str!("compile_fail_cases/lifecycle/shutdown_while_device_borrowed.rs"),
        error_code: "E0505",
    },
    CompileFailCase {
        bin: "exhaustive_device_state_match",
        source: include_str!("compile_fail_cases/public_api/exhaustive_device_state_match.rs"),
        error_code: "E0004",
    },
    CompileFailCase {
        bin: "removed_callback_retired_state",
        source: include_str!("compile_fail_cases/public_api/removed_callback_retired_state.rs"),
        error_code: "E0599",
    },
];

#[test]
fn compile_time_contracts_fail_with_the_expected_error_codes() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));
    let project = CompileFailProject::new(&workspace)
        .unwrap_or_else(|error| panic!("failed to prepare compile-fail project: {error}"));

    let output = Command::new(&cargo)
        .arg("generate-lockfile")
        .arg("--offline")
        .arg("--manifest-path")
        .arg(project.manifest())
        .env("CARGO_TARGET_DIR", project.target())
        .output()
        .unwrap_or_else(|error| panic!("failed to generate compile-fail lockfile: {error}"));

    assert!(
        output.status.success(),
        "failed to generate compile-fail lockfile\nCargo stdout:\n{}\nCargo stderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    for case in CASES {
        let output = Command::new(&cargo)
            .arg("check")
            .arg("--frozen")
            .arg("--manifest-path")
            .arg(project.manifest())
            .arg("--bin")
            .arg(case.bin)
            .arg("--message-format=json")
            .env("CARGO_TARGET_DIR", project.target())
            .output()
            .unwrap_or_else(|error| panic!("failed to run Cargo for `{}`: {error}", case.bin));

        assert!(
            !output.status.success(),
            "`{}` unexpectedly compiled; the compile-time contract is no longer enforced",
            case.bin
        );

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let (codes, has_uncoded_error) = compiler_error_codes(&stdout);
        let expected = BTreeSet::from([case.error_code.to_owned()]);

        assert!(
            !has_uncoded_error && codes == expected,
            "`{}` failed for an unexpected reason\nexpected error codes: {expected:?}\nactual error codes: {codes:?}\nuncoded error: {has_uncoded_error}\nCargo stderr:\n{stderr}\nCargo JSON:\n{stdout}",
            case.bin
        );
    }
}

struct CompileFailProject {
    root: PathBuf,
}

impl CompileFailProject {
    fn new(workspace: &Path) -> io::Result<Self> {
        let project = Self {
            root: create_scratch_dir()?,
        };
        let bins = project.root.join("src/bin");
        fs::create_dir_all(&bins)?;

        let dependency_path = format!("{:?}", workspace.to_string_lossy());
        fs::write(
            project.manifest(),
            format!(
                "[package]\n\
                 name = \"mv3d-lp-compile-fail\"\n\
                 version = \"0.0.0\"\n\
                 edition = \"2024\"\n\
                 publish = false\n\n\
                 [workspace]\n\n\
                 [dependencies]\n\
                 mv3d-lp = {{ path = {dependency_path}, default-features = false }}\n"
            ),
        )?;

        for case in CASES {
            fs::write(bins.join(format!("{}.rs", case.bin)), case.source)?;
        }

        Ok(project)
    }

    fn manifest(&self) -> PathBuf {
        self.root.join("Cargo.toml")
    }

    fn target(&self) -> PathBuf {
        self.root.join("target")
    }
}

impl Drop for CompileFailProject {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn create_scratch_dir() -> io::Result<PathBuf> {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    let base = option_env!("CARGO_TARGET_TMPDIR")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    fs::create_dir_all(&base)?;

    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let candidate = base.join(format!("compile-fail-{}-{id}", std::process::id()));

        match fs::create_dir(&candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

fn compiler_error_codes(cargo_json: &str) -> (BTreeSet<String>, bool) {
    const CODE_PREFIX: &str = "\"code\":{\"code\":\"";
    const UNCODED_ERROR: &str = "\"code\":null,\"level\":\"error\"";

    let mut codes = BTreeSet::new();
    let mut has_uncoded_error = false;

    for line in cargo_json
        .lines()
        .filter(|line| line.contains("\"reason\":\"compiler-message\""))
    {
        has_uncoded_error |= line.contains(UNCODED_ERROR);

        let mut remaining = line;
        while let Some((_, after_prefix)) = remaining.split_once(CODE_PREFIX) {
            let Some((code, tail)) = after_prefix.split_once('"') else {
                break;
            };
            codes.insert(code.to_owned());
            remaining = tail;
        }
    }

    (codes, has_uncoded_error)
}
