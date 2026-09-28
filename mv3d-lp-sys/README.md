# mv3d-lp-sys

Unofficial raw FFI bindings for the Hikrobot 3DMVS laser profiler SDK (LPSDK, `Mv3dLp`),
baseline LPSDK 1.3.3.3. Most applications should use the safe [`mv3d-lp`](https://crates.io/crates/mv3d-lp)
crate, which re-exports this crate as `mv3d_lp::sys`.

## Requirements

- Target: `x86_64-pc-windows-msvc` only; other targets fail at compile time.
- Build: the bindings link `Mv3dLp.dll` through `raw-dylib`, so building needs neither the SDK nor its
  import library.
- Run: the directory containing `Mv3dLp.dll` must be on `PATH`.

## Bindings

The declarations are hand-written from `Mv3dLpApi.h`, `Mv3dLpDefine.h` and `Mv3dLpImgProc.h`;
deprecated profile-era interfaces are omitted. `src/layout.rs` checks every structure size and field offset
at compile time, and the item shapes follow what bindgen would generate for the same headers
(`u32` status constants, `extern "C"` callbacks, `Debug` on union-free structures).

The vendor SDK, headers, import libraries and DLLs are not redistributed.

## License

MIT
