# mv3d-lp-internal

`mv3d-lp-internal` is the private implementation crate for
[`mv3d-lp`](https://github.com/JSB-Unscarred/3dmvs-sdk-rs). It isolates the audited raw
LPSDK bindings, process-wide runtime state, callback registry, and native
resource cleanup from the public safe facade. Public types such as `Image`,
`Parameter`, and `DeviceInfo` are defined here and re-exported by `mv3d-lp`.

Production native calls, union reads, and image pointer conversion live in `ffi.rs`. Callback
trampoline entry, raw-pointer admission, and exception copying live in `callback.rs`. `bindings.rs`
contains the raw declarations and is the single source for every SDK constant; `abi.rs` checks
layouts of structures used by the safe wrapper. `build.rs` publishes the `sdk_target` and
`native_sdk` cfg aliases so the target/feature predicate is written once.

注释分工：调用方可见的功能与调用约定写在 `mv3d-lp` 一侧，本 crate 的注释只说明为防止什么问题
引入了什么安全设计。两侧各写一份完整契约会各自演化，此前 callback panic 语义的漂移即由此产生。

Registry publication is currently disabled. If the public facade is published in the future,
this implementation crate must be published first because registry packages cannot depend on an
unpublished path dependency. Its API remains an implementation detail: applications should depend
on `mv3d-lp`, and no compatibility promise is made for direct use of `mv3d-lp-internal`.

The default feature set is empty and requires no vendor SDK. Native linking is
available only through the explicit `native` feature on
`x86_64-pc-windows-msvc`. The bindings baseline is LPSDK `1.3.3.3`;
`GetVersion` is returned as raw text and initialization does not impose a wrapper version range.
The vendor SDK, headers, import libraries, DLLs, and installers are not included
or redistributed.

Licensed under the repository's [MIT License](../LICENSE).
