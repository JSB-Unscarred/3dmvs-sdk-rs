# Security Policy

## Supported versions

The project is currently preparing its first `0.1.0` release. Until a release
is published, only the latest commit on `main` receives fixes. After release,
the latest `0.1.x` version will receive security fixes; older pre-1.0 versions
may be yanked when a soundness or ABI issue makes continued use unsafe.

## Reporting a vulnerability

Please report suspected memory-safety, FFI, callback-lifetime, ABI, resource
cleanup, path-handling, or device-control vulnerabilities privately through
[GitHub Security Advisories](https://github.com/JSB-Unscarred/3dmvs-sdk-rs/security/advisories/new).
Do not open a public issue before maintainers have had a reasonable opportunity
to assess and contain the problem.

Include, when available:

- the affected crate version or commit and enabled features;
- Rust target, toolchain, Windows and exact LPSDK/runtime versions;
- device and firmware information with serial numbers, IP addresses, paths,
  credentials, and customer data removed;
- a minimal reproducer, expected behavior, observed behavior, and whether the
  problem reproduces without hardware through the fake backend;
- panic, sanitizer, Miri, callback, Drop-order, or native logs that do not
  redistribute vendor files.

Maintainers will acknowledge a usable report, assess affected versions and
coordinate a fix or mitigation. This volunteer project does not promise a
fixed response SLA. Once a fix is available, the project may publish an
advisory, yank affected crates.io versions, and release a new patch or minor
version. Published tags and crate contents will not be overwritten.

## Scope and trust boundary

The MIT license covers only this repository. Hikrobot/3DMVS headers, import
libraries, DLLs, runtimes, installers, devices, firmware, and network services
remain third-party components and are not redistributed here.

The Rust test suite can validate wrapper behavior under the documented Driver
contract. It cannot inspect closed-source DLL threads or turn device testing
into a vendor guarantee. Known native assumptions and evidence limits are
tracked in [`m5/native-contract-evidence.md`](m5/native-contract-evidence.md).
A report showing that one of those assumptions is false is a security-relevant
contract failure even when no exploit has yet been demonstrated.
