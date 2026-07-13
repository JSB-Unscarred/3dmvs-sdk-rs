# mv3d-lp-internal

`mv3d-lp-internal` is the private implementation crate for
[`mv3d-lp`](https://github.com/JSB-Unscarred/3dmvs-sdk-rs). It isolates the audited raw
LPSDK bindings, process-wide runtime state, callback registry, and native
resource cleanup from the public safe facade.

Registry publication is currently disabled. If the public facade is published in the future,
this implementation crate must be published first because registry packages cannot depend on an
unpublished path dependency. Its API remains an implementation detail: applications should depend
on `mv3d-lp`, and no compatibility promise is made for direct use of `mv3d-lp-internal`.

The default feature set is empty and requires no vendor SDK. Native linking is
available only through the explicit `native` feature on
`x86_64-pc-windows-msvc`, with the audited LPSDK runtime version `1.3.3.3`.
The vendor SDK, headers, import libraries, DLLs, and installers are not included
or redistributed.

Licensed under the [MIT License](LICENSE).
