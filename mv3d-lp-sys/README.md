# mv3d-lp-sys

Raw, unsafe FFI bindings for the Hikrobot 3DMVS laser profiler SDK (LPSDK `1.3.3.3`).
The declarations are hand-audited against the vendor headers for Windows x86_64 MSVC, and
`src/layout.rs` checks structure sizes and offsets at compile time. Application code should
normally depend on the safe `mv3d-lp` crate.

The build script reads `MV3DLP_DEV_ENV` (default
`C:\Program Files (x86)\3DMVS\Development`) and links `Libraries/win64/Mv3dLp.lib`. Without the
SDK it only emits a warning, so `cargo check`, `clippy` and `doc` still run.

The vendor SDK, headers, import libraries and DLLs are not redistributed.

Licensed under the repository's [MIT License](../LICENSE).
