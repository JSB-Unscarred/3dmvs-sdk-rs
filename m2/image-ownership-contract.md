# M2 pull acquisition ownership contract

This document records the memory assumptions used by the safe `GetImage -> OwnedFrame`
boundary for the audited LPSDK runtime `1.3.3.3`.

## Decision

The project accepts the existing M0 audit's
"SDK-owned; copy immediately" rule as the native contract for runtime `1.3.3.3`, including
the minimum stable read window needed to perform that copy. This is an explicit project
assumption, not a claim that separate written vendor confirmation has been obtained.

The project has no available separate written vendor confirmation of the stable read window.
M5 therefore records this as a disclosed, non-blocking residual assumption for the exact
audited environment rather than describing it as a vendor guarantee. Lack of confirmation
alone does not block the `0.1.0` release, but evidence that contradicts the assumption does:
in that case the safe `get_image` surface must be disabled or redesigned before another release.

## Evidence and status

- `m0/api-contracts.json` classifies the pointers returned by `MV3D_LP_GetImage` as
  SDK-owned and requires every payload to be copied immediately.
- The installed public header declares `pData`, `pIntensityData`, and
  `pExposureTimeStamp` as output pointers. It defines the first two lengths in bytes and
  the exposure timestamp element count as `nHeight`.
- The installed `SimpleView_FetchFrame` sample neither allocates nor frees the returned
  image buffers, which is consistent with SDK ownership.
- The public header and sample do not separately document whether an SDK acquisition
  thread may rewrite a returned buffer while the caller is copying it. No claim of a
  separate written vendor confirmation is made here.

The safe wrapper therefore relies on the following FFI precondition: after a successful
`MV3D_LP_GetImage` call, every non-null returned payload remains readable and is not
concurrently modified until the caller has completed an immediate synchronous copy,
provided that the caller makes no intervening SDK call for that process.

This precondition is a disclosed project assumption in the audited native contract, not a
separate vendor guarantee. If new vendor material or an observation contradicts it, the safe
`get_image` API must be disabled or redesigned; a Rust mutex cannot prevent an undocumented
vendor worker thread from writing the same allocation.

## Pointer and invalidation rules

- The SDK owns all three returned allocations. Rust never frees or writes them.
- `pData` covers `nDataLen` bytes. A non-zero length with a null pointer is rejected.
- `pIntensityData` covers `nIntensityDataLen` bytes. A zero length is represented as
  `None`; a non-zero length with a null pointer is rejected.
- A non-null `pExposureTimeStamp` covers exactly `nHeight` native `i64` values. A null
  pointer is represented as `None` because the public material does not state that this
  auxiliary payload is present for every image mode.
- The copy is completed inside `NativeDriver::get_image`, before the process-wide SDK
  lock is released. No raw descriptor or pointer crosses the Driver boundary.
- A later GetImage, ClearDataBuffer, StopMeasure, CloseDevice, or SDK shutdown cannot
  affect an `OwnedFrame` because all payloads have already been copied.

## Descriptor validation

On a successful SDK status, the wrapper validates the entire descriptor before it reads
any payload:

- dimensions and all size calculations use checked arithmetic;
- known uncompressed formats must contain at least the tightly packed minimum of 1, 2,
  3, 6, or 12 bytes per pixel as defined by their audited image type; extra bytes are
  retained as potential row padding because M2 exposes no structured pixel view;
- JPEG and unknown future image types use the reported byte length but still receive
  pointer, arithmetic, allocation, and aggregate-size checks;
- an intensity plane, when present, is tightly packed Mono8 (`width * height` bytes);
- the aggregate owned payload is limited by the audited frame-size limit;
- every allocation is performed fallibly before any SDK pointer is dereferenced.

On a non-success SDK status, all descriptor fields are ignored. In particular, timeout
and disconnection paths never inspect output pointers that the SDK was not required to
initialize.

## Deferred questions

The following points should still be confirmed against vendor support material when it
becomes available and this document updated with the evidence:

1. The precise invalidation event for each returned allocation.
2. Whether intensity and exposure timestamp pointers are guaranteed non-null in specific
   image modes.
3. Whether every supported uncompressed mode is guaranteed to have no row padding.
4. The maximum supported batch height and corresponding worst-case aggregate frame size.
