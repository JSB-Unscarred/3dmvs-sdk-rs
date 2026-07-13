# Changelog

All notable changes to this project will be documented in this file. The format
is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this
project follows Semantic Versioning for its public crate.

## [Unreleased]

### Added

- M5 hardening and release documentation, native-contract evidence ledger, and
  a repeatable release checklist.
- MIT licensing and future registry metadata for the public facade and its internal
  implementation crate; actual publication remains disabled.
- Hardware-independent CI coverage for formatting, linting, cross-platform
  tests, MSRV, rustdoc, compile-fail checks, auto-trait assertions, and Miri.
- No-SDK raw-symbol stubs that exercise production `NativeDriver` FFI status
  gates and poisoned out-parameter handling for every production call point.

### Changed

- The default feature set is now empty. Applications must explicitly enable
  `native` to link LPSDK, while ordinary builds and tests require no vendor SDK.
- `mv3d-lp` uses an exact-version dependency on `mv3d-lp-internal`; the two
  crates must keep synchronized versions if registry publication resumes.

### Known limitations

- Native operation is limited to `x86_64-pc-windows-msvc` and exactly LPSDK
  `1.3.3.3`.
- Separate vendor guarantees are unavailable for several buffer-stability,
  callback-quiescence, background-file-access, and input-retention assumptions.
  See `m5/native-contract-evidence.md`; industrial experiments are environment
  observations and are not vendor guarantees.

[Unreleased]: https://github.com/JSB-Unscarred/3dmvs-sdk-rs/commits/main
