use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::Command;

struct CompileFailCase {
    bin: &'static str,
    error_code: &'static str,
}

const CASES: &[CompileFailCase] = &[
    CompileFailCase {
        bin: "non_send_handler",
        error_code: "E0277",
    },
    CompileFailCase {
        bin: "non_static_handler",
        error_code: "E0521",
    },
    CompileFailCase {
        bin: "camera_outlives_sdk",
        error_code: "E0515",
    },
    CompileFailCase {
        bin: "close_while_measuring",
        error_code: "E0505",
    },
    CompileFailCase {
        bin: "image_ref_outlives_payload",
        error_code: "E0515",
    },
    CompileFailCase {
        bin: "reborrow_camera_while_measuring",
        error_code: "E0499",
    },
    CompileFailCase {
        bin: "reborrow_camera_while_transferring",
        error_code: "E0499",
    },
    CompileFailCase {
        bin: "shutdown_while_camera_borrowed",
        error_code: "E0505",
    },
];

#[test]
fn safety_contracts_fail_with_the_expected_error_codes() {
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let manifest = workspace.join("tests/compile_fail_cases/Cargo.toml");
    let target = workspace.join("target/compile-fail");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"));

    for case in CASES {
        let output = Command::new(&cargo)
            .arg("check")
            .arg("--frozen")
            .arg("--manifest-path")
            .arg(&manifest)
            .arg("--bin")
            .arg(case.bin)
            .arg("--message-format=json")
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .unwrap_or_else(|error| panic!("failed to run Cargo for `{}`: {error}", case.bin));

        assert!(
            !output.status.success(),
            "`{}` unexpectedly compiled; the safety contract is no longer enforced",
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
