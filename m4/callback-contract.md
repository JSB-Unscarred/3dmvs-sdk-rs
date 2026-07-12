# M4 callback ownership and lifecycle contract

This document records the safe boundary used for the LPSDK 1.3.3.3 image and
device-exception callbacks. It supplements the pull-acquisition ownership
contract in `m2/image-ownership-contract.md`.

## Decision

M4 exposes callback data only as owned Rust values. The primary image surface
returns a `Receiver<OwnedFrame>` configured by `CallbackOptions`. A closure
adapter is layered on top of the receiver and invokes the user's `FnMut` only
on a named Rust worker thread. No user closure runs on an SDK callback thread.

Both image and exception callbacks use the same process-wide registry and
opaque-cookie lifecycle. The implementation does not rely on the SDK to
unregister a callback or to quiesce callbacks when measurement stops or a
device closes.

## Audited vendor ABI and evidence

The two public registration functions are:

```text
MV3D_LP_STATUS __cdecl MV3D_LP_RegisterImageDataCallBack(
    HANDLE,
    MV3D_LP_ImageDataCallBack,
    void *pUser);

MV3D_LP_STATUS __cdecl MV3D_LP_RegisterExceptionCallBack(
    HANDLE,
    MV3D_LP_ExceptionCallBack,
    void *pUser);
```

Their callback types are:

```text
void __stdcall image(MV3D_LP_IMAGE_DATA *, void *pUser);
void __stdcall exception(MV3D_LP_EXCEPTION_INFO *, void *pUser);
```

Rust therefore declares the registration functions as `extern "C"` and the
callbacks as `extern "system"`; the distinction is material on 32-bit Windows
even though the native crate currently supports only x86-64 Windows MSVC.

The installed headers call `pUser` a user-defined variable, and vendor samples
round-trip a device handle or C++ `this` pointer through it. The C guide and
samples register callbacks after opening the device and before
`MV3D_LP_StartMeasure`.

Neither the public headers nor the guide declares an unregister function. The
installed x86 and x64 DLL export tables likewise contain no callback-unregister
symbol. Passing a null callback to a registration function is representable by
the Rust ABI type, but the vendor does not document it as unregistration and
M4 does not use it as a safety mechanism.

The vendor does not document:

- whether callbacks are serialized, concurrent, or reentrant;
- callback ordering across SDK threads or devices;
- whether `StopMeasure` or `CloseDevice` waits for in-flight callbacks;
- how long the SDK retains the callback and `pUser` values;
- replacement semantics for a second registration on the same handle.

The guide advises against calling SDK functions from the image callback. M4
calls no SDK API from either native trampoline.

## Registry and opaque cookies

Each native registration receives a non-zero, process-unique cookie encoded in
`pUser`. A cookie is only an address-valued identifier: Rust never dereferences
it as a pointer. Cookie values are not reused during the process lifetime.
Consequently, a callback delayed from an old registration cannot resolve to a
new registration through an ABA collision.

A synchronized process-wide registry maps active cookies to reference-counted
callback entries. Registration follows this order:

1. allocate a fresh cookie and complete the Rust entry;
2. publish the entry in the registry;
3. call the vendor registration function with the static trampoline and cookie;
4. if the vendor call fails, deactivate and remove the entry.

Publishing first permits an SDK that invokes the callback synchronously from
the registration call to resolve the cookie. Treating a failed registration as
potentially partial ensures that a later unexpected callback is still harmless.

A trampoline performs registry lookup and clones the entry while holding the
registry lock, then releases that lock before validating, copying, or sending
the event. Unknown, zero, inactive, or stale cookies are ignored before the
payload pointer is inspected.

Teardown first marks the entry inactive and removes its cookie from the
registry, independently of whether later `StopMeasure` or `CloseDevice` calls
succeed. A callback that already cloned an entry may finish safely because the
entry is reference counted. Teardown waits for every admitted callback to
finish, so an admitted callback may publish before teardown returns but none
can publish after it returns. A revoked entry and an unknown late callback can
never reach freed Rust state. Already queued owned events may still be drained
after revocation; revocation does not retroactively remove them from a receiver.

The safe facade performs at most one native registration per callback kind and
device handle. A new Rust consumer, when supported, changes only the Rust-side
sink; it does not assume undocumented native replacement semantics.

`Camera` always owns both native callback registrations. A
`CallbackMeasurement` only borrows the camera's image-registration slot; it
does not become the sole owner of the registry entry. Consequently, safe code
that calls `mem::forget` on the guard cannot prevent `Camera` cleanup from
revoking and draining the image cookie before Stop/Close.

Callback acquisition uses a distinct `CallbackMeasuring` camera state. If its
guard is forgotten, methods that are valid for ordinary pull measurement—most
notably `ClearDataBuffer`, parameter access, and command execution—remain
unavailable. The camera can still be explicitly closed, which performs the
registry revocation and native cleanup sequence.

## Image payload ownership

`MV3D_LP_IMAGE_DATA` contains the primary byte pointer and length, an optional
intensity byte pointer and length, and an optional exposure-timestamp pointer.
The header defines the exposure timestamp count as `nHeight`. Dimensions,
format, frame number, device timestamp, validity, scale, and offset metadata
are stored directly in the descriptor.

The accepted native precondition for M4 is that, for the duration of an image
callback invocation, the descriptor and every reported non-null payload remain
readable and are not concurrently modified by the SDK. This is consistent with
the callback API and vendor samples, which inspect and copy the data inside the
callback, but it is not a separately documented vendor guarantee. If this
precondition is false, a safe owned callback API cannot be provided.

The image trampoline completes all of the following before returning to the
SDK:

1. reject a null descriptor;
2. validate dimensions, pointer/length pairs, checked size arithmetic, known
   format minimum lengths, and the aggregate frame-size limit;
3. perform fallible allocations for all present payloads;
4. copy the primary data, intensity data, and `nHeight` native `i64` exposure
   timestamps;
5. construct a `FrameRecord` and publish only that owned record.

The conversion rules and 512 MiB aggregate limit are shared with M2 pull
acquisition. No SDK descriptor, borrowed slice, or raw payload pointer crosses
the private FFI boundary. A malformed descriptor or failed allocation is
contained in the trampoline and produces no `OwnedFrame`.

## Exception payload ownership

`MV3D_LP_EXCEPTION_INFO` contains an integer exception type, a fixed
`char[256]` description, and four reserved bytes. The public enum currently
defines `Undefined` (`-1`) and `Disconnect` (`1`); unknown future integer values
are retained rather than interpreted as a Rust enum discriminant.

The exception trampoline rejects a null descriptor and copies the complete
event before returning. The vendor does not document the description encoding
or guarantee NUL termination. M4 therefore treats it as bounded SDK text:
bytes through the first NUL are retained, and all 256 bytes are retained when
there is no NUL. It does not perform an unbounded `CStr::from_ptr` scan or
assume UTF-8.

Exception receivers and closure adapters use the same registry, cookie,
revocation, late-callback, queue, worker, and panic rules as image callbacks.

## Queueing, workers, and concurrency

`CallbackOptions` configures a bounded `sync_channel`; its default capacity is
four events and registration rejects capacities above 64 before channel
allocation. Native trampolines use non-blocking `try_send`. When the queue is
full, the newest event is dropped, and when the receiver is disconnected the
owned event is discarded. The SDK callback thread is never blocked waiting for
Rust consumer work. This is an intentional lossy live-stream contract, not a
lossless recording interface.

The SDK may call a trampoline concurrently from arbitrary foreign threads.
Registry access and entry state are synchronized, and no total order is
promised for events submitted concurrently. Each `Receiver` yields only fully
owned values and may be moved to a consumer thread.

`CallbackWorker` owns a receiver and invokes its handler serially on one named
Rust thread. The worker loop isolates a handler panic with `catch_unwind`, then
terminates that worker. Joining reports `CallbackWorkerExit::HandlerPanicked`;
dropping the worker handle detaches it. User handler execution never occurs on
the vendor callback stack.

## Panic and FFI isolation

Each `extern "system"` trampoline has an outer `catch_unwind` boundary covering
cookie handling, registry lookup, payload validation and copying, and channel
publication. No Rust panic may cross the C ABI boundary. A panic puts that
registration into a fail-closed state and disconnects its Rust sink after
in-flight work exits; it does not invoke user code or unwind into the SDK.

Registry poison is recovered without panicking. All destructor work reachable
from a native trampoline remains inside the unwind boundary. Handler panics are
separately isolated on the Rust worker as described above.

## Required tests

M4's hardware-independent callback tests cover at least:

- exact callback ABI type checks;
- primary and auxiliary image payload ownership after source mutation;
- bounded exception-description conversion, including no-NUL input and unknown
  exception values;
- concurrent invocations for the same and different cookies;
- queue-full drop-newest behavior without blocking the calling thread;
- disconnected receivers;
- null payloads, malformed image descriptors, unknown cookies, and zero cookies;
- callbacks racing deactivation and callbacks arriving after teardown;
- stale-cookie rejection without cookie reuse;
- a panic injected inside trampoline work never escaping the FFI boundary;
- closure execution on a Rust worker rather than the invoking callback thread;
- handler panic reporting through `CallbackWorkerExit::HandlerPanicked`;
- image and exception callbacks following the same lifecycle rules.

## Residual vendor questions

Before publishing the callback facade beyond the audited 1.3.3.3 environment,
obtain vendor confirmation of:

1. the stable read window for every callback payload;
2. the maximum callback concurrency and whether callbacks may be reentrant;
3. the effect of `StopMeasure` and `CloseDevice` on queued and in-flight calls;
4. whether a supported native unregister operation exists but is undocumented;
5. the encoding and termination rules for exception descriptions.

The registry/cookie design makes undocumented late delivery memory-safe, but it
cannot repair an SDK that mutates a payload while the SDK itself is calling the
user callback.
