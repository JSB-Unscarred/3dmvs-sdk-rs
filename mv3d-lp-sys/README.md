# mv3d-lp-sys

Unofficial raw FFI bindings for the Hikrobot 3DMVS laser profiler SDK (LPSDK, `Mv3dLp`).
Most applications should use the safe [`mv3d-lp`](https://crates.io/crates/mv3d-lp) crate, which re-exports
this crate as `mv3d_lp::sys`.

- Target: `x86_64-pc-windows-msvc` only.
- Building does not need the SDK: the declarations are written from the LPSDK 1.3.3.3 headers, with every
  structure layout checked at compile time, and link `Mv3dLp.dll` through `raw-dylib`.
- At run time, LPSDK 1.3.3.3 or later must be installed and the directory of `Mv3dLp.dll` must be on `PATH`.
- Deprecated profile-era interfaces are not declared.

The vendor SDK, headers, import libraries and DLLs are not redistributed.

## License

MIT
