#[test]
fn safety_contracts_do_not_compile() {
    if std::env::var_os("MV3DLP_RUN_UI").is_none() {
        return;
    }
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/lifecycle/*.rs");
    tests.compile_fail("tests/ui/threading/*.rs");
    tests.compile_fail("tests/ui/callback/*.rs");
}
