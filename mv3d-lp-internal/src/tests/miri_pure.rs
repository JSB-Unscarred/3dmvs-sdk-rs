//! Pure-Rust smoke tests for Miri.
//!
//! These tests use only Rust-owned backing storage and the injected fake driver. They must not
//! enable the `native` feature or load the vendor SDK. Run them, together with the rest of the
//! internal unit tests, using:
//!
//! `cargo +nightly-2026-07-09 miri test -p mv3d-lp-internal --lib --no-default-features --locked`

use std::net::Ipv4Addr;

use crate::bindings::{ImageType_Mono8, MV3D_LP_OK};
use crate::ffi::{FrameLimits, callback_image_from_native, image_from_test_buffers, zeroed_image};

use super::mock_driver::{MockDriver, active_runtime};

#[test]
fn raw_descriptor_payloads_are_copied_from_live_rust_storage() {
    let mut data = [1, 2, 3, 4];
    let mut intensity = [5, 6, 7, 8];
    let mut exposure_timestamps = [10_i64, 20];
    let mut image = zeroed_image();
    image.enImageType = ImageType_Mono8;
    image.nWidth = 2;
    image.nHeight = 2;
    image.nDataLen = data.len() as u32;
    image.nIntensityDataLen = intensity.len() as u32;

    let frame = image_from_test_buffers(
        MV3D_LP_OK,
        image,
        Some(&data),
        Some(&intensity),
        Some(&exposure_timestamps),
        FrameLimits::default(),
    )
    .unwrap();

    data.fill(0);
    intensity.fill(0);
    exposure_timestamps.fill(0);

    assert_eq!(frame.data, [1, 2, 3, 4]);
    assert_eq!(frame.intensity_data, Some(vec![5, 6, 7, 8]));
    assert_eq!(frame.exposure_timestamps, Some(vec![10, 20]));
}

#[test]
fn unaligned_exposure_storage_is_copied_as_native_bytes() {
    let mut data = [7_u8];
    let expected = 0x0102_0304_0506_0708_i64;
    let mut exposure_storage = [0_u8; 9];
    exposure_storage[1..].copy_from_slice(&expected.to_ne_bytes());

    let mut image = zeroed_image();
    image.enImageType = ImageType_Mono8;
    image.nWidth = 1;
    image.nHeight = 1;
    image.pData = data.as_mut_ptr();
    image.nDataLen = 1;
    // SAFETY: `exposure_storage` contains eight initialized bytes starting at offset one. The
    // conversion deliberately reads this pointer as bytes and never dereferences it as `i64`.
    image.pExposureTimeStamp = unsafe { exposure_storage.as_mut_ptr().add(1).cast::<i64>() };

    // SAFETY: Every descriptor pointer is backed by the live arrays above for the complete call.
    let frame = unsafe { callback_image_from_native(&image) }.unwrap();
    assert_eq!(frame.data, [7]);
    assert_eq!(frame.exposure_timestamps, Some(vec![expected]));
}

#[test]
fn fake_backend_exercises_drop_cleanup_without_native_ffi() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);

    {
        let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
        let measurement = camera.start().unwrap();
        drop(measurement);
    }
    runtime.shutdown().unwrap();

    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "open_by_ip",
            "start",
            "stop",
            "close",
            "finalize"
        ]
    );
}
