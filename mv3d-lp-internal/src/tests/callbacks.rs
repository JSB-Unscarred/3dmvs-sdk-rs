use std::net::Ipv4Addr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Barrier, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::bindings::{
    DevExceptionType_Disconnect, ImageType_Mono8, MV3D_LP_EXCEPTION_INFO, MV3D_LP_IMAGE_DATA,
    MV3D_LP_MAX_STRING_LENGTH,
};
use crate::callback::{
    CallbackDelivery, CallbackRegistration, ExceptionCallbackSink, ExceptionRecord,
    FrameCallbackSink, invoke_exception_for_test, invoke_image_for_test,
};
use crate::driver::{DriverError, DriverResult};
use crate::error::Error;
use crate::frame::FrameRecord;

use super::mock_driver::{MockDriver, active_runtime};

#[test]
fn image_callback_immediately_copies_all_three_payloads_and_metadata() {
    let (sender, receiver) = mpsc::channel::<FrameRecord>();
    let sink: FrameCallbackSink = Arc::new(move |frame| {
        sender.send(frame).unwrap();
        CallbackDelivery::Delivered
    });
    let registration = CallbackRegistration::image(sink).unwrap();

    let mut data = vec![1, 2, 3, 4];
    let mut intensity = vec![5, 6, 7, 8];
    let mut exposure = vec![1_000_i64, 2_000];
    let mut image = crate::ffi::zeroed_image();
    image.enImageType = ImageType_Mono8;
    image.nWidth = 2;
    image.nHeight = 2;
    image.pData = data.as_mut_ptr();
    image.nDataLen = data.len() as u32;
    image.pIntensityData = intensity.as_mut_ptr();
    image.nIntensityDataLen = intensity.len() as u32;
    image.pExposureTimeStamp = exposure.as_mut_ptr();
    image.nFrameNum = 42;
    image.nTimeStamp = 123_456;
    image.bValid = -1;
    image.fXScale = 0.1;
    image.fYScale = 0.2;
    image.fZScale = 0.3;
    image.nXOffset = -10;
    image.nYOffset = 20;
    image.nZOffset = -30;

    invoke_image_for_test(registration.cookie(), &mut image);
    data.fill(0);
    intensity.fill(0);
    exposure.fill(0);

    let frame = receiver.recv().unwrap();
    assert_eq!(frame.data, [1, 2, 3, 4]);
    assert_eq!(frame.intensity_data, Some(vec![5, 6, 7, 8]));
    assert_eq!(frame.exposure_timestamps, Some(vec![1_000, 2_000]));
    assert_eq!(frame.frame_number, 42);
    assert_eq!(frame.device_timestamp, 123_456);
    assert!(frame.valid);
    assert_eq!(
        (frame.x_scale, frame.y_scale, frame.z_scale),
        (0.1, 0.2, 0.3)
    );
    assert_eq!(
        (frame.x_offset, frame.y_offset, frame.z_offset),
        (-10, 20, -30)
    );
    assert_eq!(registration.stats().delivered, 1);
}

#[test]
fn exception_callback_bounds_description_and_preserves_unknown_kind() {
    let (sender, receiver) = mpsc::channel::<ExceptionRecord>();
    let sink: ExceptionCallbackSink = Arc::new(move |exception| {
        sender.send(exception).unwrap();
        CallbackDelivery::Delivered
    });
    let registration = CallbackRegistration::exception(sink).unwrap();

    let mut terminated = exception_descriptor(DevExceptionType_Disconnect);
    copy_exception_description(&mut terminated, b"offline\0ignored");
    invoke_exception_for_test(registration.cookie(), &mut terminated);
    terminated.chExceptionDesc.fill(b'X' as i8);

    let unknown_kind = 0x1234_5678;
    let mut unterminated = exception_descriptor(unknown_kind);
    unterminated.chExceptionDesc.fill(0xFF_u8 as i8);
    invoke_exception_for_test(registration.cookie(), &mut unterminated);

    assert_eq!(
        receiver.recv().unwrap(),
        ExceptionRecord {
            kind: DevExceptionType_Disconnect,
            description: b"offline".to_vec(),
        }
    );
    assert_eq!(
        receiver.recv().unwrap(),
        ExceptionRecord {
            kind: unknown_kind,
            description: vec![0xFF; MV3D_LP_MAX_STRING_LENGTH],
        }
    );
    assert_eq!(registration.stats().delivered, 2);
}

#[test]
fn concurrent_callbacks_are_delivered_once_without_crossing_cookies() {
    const CALLBACKS: usize = 32;

    let (first_sender, first_receiver) = mpsc::channel::<u32>();
    let first_sink: FrameCallbackSink = Arc::new(move |frame| {
        first_sender.send(frame.frame_number).unwrap();
        CallbackDelivery::Delivered
    });
    let first = CallbackRegistration::image(first_sink).unwrap();

    let (second_sender, second_receiver) = mpsc::channel::<u32>();
    let second_sink: FrameCallbackSink = Arc::new(move |frame| {
        second_sender.send(frame.frame_number).unwrap();
        CallbackDelivery::Delivered
    });
    let second = CallbackRegistration::image(second_sink).unwrap();

    let start = Arc::new(Barrier::new(CALLBACKS + 1));
    let mut threads = Vec::with_capacity(CALLBACKS);
    for index in 0..CALLBACKS {
        let start = Arc::clone(&start);
        let cookie = if index % 2 == 0 {
            first.cookie()
        } else {
            second.cookie()
        };
        threads.push(thread::spawn(move || {
            start.wait();
            invoke_mono(cookie, index as u32, index as u8);
        }));
    }
    start.wait();
    for thread in threads {
        thread.join().unwrap();
    }

    let mut first_frames: Vec<_> = first_receiver.try_iter().collect();
    let mut second_frames: Vec<_> = second_receiver.try_iter().collect();
    first_frames.sort_unstable();
    second_frames.sort_unstable();
    assert_eq!(
        first_frames,
        (0..CALLBACKS as u32)
            .filter(|number| number % 2 == 0)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        second_frames,
        (0..CALLBACKS as u32)
            .filter(|number| number % 2 == 1)
            .collect::<Vec<_>>()
    );
    assert_eq!(first.stats().delivered, (CALLBACKS / 2) as u64);
    assert_eq!(second.stats().delivered, (CALLBACKS / 2) as u64);
}

#[test]
fn deactivation_rejects_new_admissions_and_waits_for_in_flight_callback() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let (entered_sender, entered_receiver) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let sink_release = Arc::clone(&release);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        if sink_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            entered_sender.send(()).unwrap();
            let (released, wake) = &*sink_release;
            let mut released = released
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            while !*released {
                released = wake
                    .wait(released)
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
            }
        }
        CallbackDelivery::Delivered
    });
    let mut registration = CallbackRegistration::image(sink).unwrap();
    let cookie = registration.cookie();

    let callback = thread::spawn(move || invoke_mono(cookie, 1, 1));
    entered_receiver.recv().unwrap();

    let (started_sender, started_receiver) = mpsc::channel();
    let (done_sender, done_receiver) = mpsc::channel();
    let deactivate = thread::spawn(move || {
        started_sender.send(()).unwrap();
        registration.deactivate();
        done_sender.send(()).unwrap();
    });
    started_receiver.recv().unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let before = calls.load(Ordering::SeqCst);
        invoke_mono(cookie, 2, 2);
        if calls.load(Ordering::SeqCst) == before {
            break;
        }
        if Instant::now() >= deadline {
            release_callback(&release);
            panic!("deactivation did not stop admitting new callbacks");
        }
        thread::yield_now();
    }
    assert_eq!(done_receiver.try_recv(), Err(mpsc::TryRecvError::Empty));

    release_callback(&release);
    callback.join().unwrap();
    deactivate.join().unwrap();
    done_receiver.recv().unwrap();
    let before = calls.load(Ordering::SeqCst);
    invoke_mono(cookie, 3, 3);
    assert_eq!(calls.load(Ordering::SeqCst), before);
}

#[test]
fn full_drops_one_event_while_disconnected_fail_closes_the_slot() {
    let full_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&full_calls);
    let full_sink: FrameCallbackSink = Arc::new(move |_| {
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            CallbackDelivery::Full
        } else {
            CallbackDelivery::Delivered
        }
    });
    let full = CallbackRegistration::image(full_sink).unwrap();
    invoke_mono(full.cookie(), 1, 1);
    invoke_mono(full.cookie(), 2, 2);
    assert_eq!(full_calls.load(Ordering::SeqCst), 2);
    assert_eq!(full.stats().dropped_full, 1);
    assert_eq!(full.stats().delivered, 1);
    assert!(full.stats().accepting);

    let disconnected_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&disconnected_calls);
    let disconnected_sink: FrameCallbackSink = Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Disconnected
    });
    let disconnected = CallbackRegistration::image(disconnected_sink).unwrap();
    invoke_mono(disconnected.cookie(), 1, 1);
    invoke_mono(disconnected.cookie(), 2, 2);
    assert_eq!(disconnected_calls.load(Ordering::SeqCst), 1);
    assert_eq!(disconnected.stats().delivered, 0);
    assert!(!disconnected.stats().accepting);
}

#[test]
fn null_malformed_and_wrong_kind_descriptors_do_not_end_valid_streams() {
    let image_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&image_calls);
    let image_sink: FrameCallbackSink = Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    let image = CallbackRegistration::image(image_sink).unwrap();

    invoke_image_for_test(image.cookie(), ptr::null_mut());
    let mut malformed = crate::ffi::zeroed_image();
    malformed.enImageType = ImageType_Mono8;
    malformed.nWidth = 1;
    malformed.nHeight = 1;
    malformed.nDataLen = 1;
    invoke_image_for_test(image.cookie(), &mut malformed);
    invoke_mono(image.cookie(), 7, 0xA5);

    let exception_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&exception_calls);
    let exception_sink: ExceptionCallbackSink = Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    let exception = CallbackRegistration::exception(exception_sink).unwrap();
    invoke_exception_for_test(exception.cookie(), ptr::null_mut());

    let mut valid_exception = exception_descriptor(DevExceptionType_Disconnect);
    copy_exception_description(&mut valid_exception, b"offline\0");
    invoke_exception_for_test(image.cookie(), &mut valid_exception);
    let (mut valid_image, data) = mono_descriptor(9, 9);
    invoke_image_for_test(exception.cookie(), &mut valid_image);
    drop(data);

    assert_eq!(image_calls.load(Ordering::SeqCst), 1);
    assert_eq!(image.stats().invalid_payloads, 2);
    assert!(image.stats().accepting);
    assert_eq!(exception_calls.load(Ordering::SeqCst), 0);
    assert_eq!(exception.stats().invalid_payloads, 1);
    assert!(exception.stats().accepting);
}

#[test]
fn zero_and_unknown_cookies_are_rejected_before_payload_access() {
    let mut hostile = crate::ffi::zeroed_image();
    hostile.enImageType = ImageType_Mono8;
    hostile.nWidth = 1;
    hostile.nHeight = 1;
    hostile.pData = ptr::without_provenance_mut(1);
    hostile.nDataLen = 1;

    // SAFETY: the descriptor itself is live, but its payload is intentionally unreadable. A
    // zero or impossible registry cookie must be rejected before the trampoline inspects pData.
    unsafe {
        crate::callback::image_trampoline(&mut hostile, ptr::null_mut());
        crate::callback::image_trampoline(&mut hostile, ptr::without_provenance_mut(usize::MAX));
    }
}

#[test]
fn a_panicking_sink_is_contained_by_the_trampoline_and_fail_closed() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        panic!("intentional callback sink panic");
    });
    let registration = CallbackRegistration::image(sink).unwrap();

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        invoke_mono(registration.cookie(), 1, 1);
    }));
    assert!(outcome.is_ok(), "panic crossed the extern trampoline");
    invoke_mono(registration.cookie(), 2, 2);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(registration.stats().panics, 1);
    assert!(!registration.stats().accepting);
}

#[test]
fn a_panic_payload_with_a_panicking_destructor_cannot_cross_the_trampoline() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        std::panic::panic_any(PanicOnDrop);
    });
    let registration = CallbackRegistration::image(sink).unwrap();

    invoke_mono(registration.cookie(), 1, 1);
    invoke_mono(registration.cookie(), 2, 2);

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(registration.stats().panics, 1);
    assert!(!registration.stats().accepting);
}

#[test]
fn failed_image_registration_retires_the_cookie_before_late_delivery() {
    let mock = MockDriver::new();
    mock.push_register_image_callback(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    assert!(matches!(
        camera.start_callback(sink),
        Err(Error::Sdk { .. })
    ));
    let cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(cookie, 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    camera.close().unwrap();
    runtime.shutdown().unwrap();
}

#[test]
fn failed_exception_registration_retires_the_cookie_before_late_delivery() {
    let mock = MockDriver::new();
    mock.push_register_exception_callback(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: ExceptionCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    assert!(matches!(
        camera.register_exception_callback(sink),
        Err(Error::Sdk { .. })
    ));
    let cookie = only_cookie(mock.exception_callback_cookies());
    invoke_disconnect(cookie);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    camera.close().unwrap();
    runtime.shutdown().unwrap();
}

#[test]
fn callback_measurement_deactivates_before_explicit_and_drop_stop() {
    assert_callback_stop_order(false);
    assert_callback_stop_order(true);
}

#[test]
fn camera_close_retires_the_cookie_of_a_forgotten_callback_measurement() {
    let mock = MockDriver::new();
    let stop_entered = Arc::new(AtomicBool::new(false));
    mock.set_stop_entered(Arc::clone(&stop_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let sink_dropped = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let probe = DropBeforeDriverCall {
        driver_entered: Arc::clone(&stop_entered),
        sink_dropped: Arc::clone(&sink_dropped),
    };
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        let _keep_probe_alive = &probe;
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    let measurement = camera.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());

    std::mem::forget(measurement);
    assert!(matches!(
        camera.clear_buffer(),
        Err(Error::InvalidState { .. })
    ));
    assert!(!mock.logs().contains(&"clear_buffer"));
    camera.close().unwrap();
    assert!(sink_dropped.load(Ordering::SeqCst));
    assert!(stop_entered.load(Ordering::SeqCst));

    let mut stale = crate::ffi::zeroed_image();
    stale.enImageType = ImageType_Mono8;
    stale.nWidth = 1;
    stale.nHeight = 1;
    stale.pData = ptr::without_provenance_mut(1);
    stale.nDataLen = 1;
    invoke_image_for_test(cookie, &mut stale);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    runtime.shutdown().unwrap();
}

#[test]
fn exception_registration_deactivates_before_close_and_survives_late_callbacks() {
    assert_exception_close_order(Ok(()));
    assert_exception_close_order(Err(DriverError::Status(0x8006_0000_u32 as i32)));
}

fn assert_callback_stop_order(drop_measurement: bool) {
    let mock = MockDriver::new();
    let stop_entered = Arc::new(AtomicBool::new(false));
    mock.set_stop_entered(Arc::clone(&stop_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let sink_dropped = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let probe = DropBeforeDriverCall {
        driver_entered: Arc::clone(&stop_entered),
        sink_dropped: Arc::clone(&sink_dropped),
    };
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        let _keep_probe_alive = &probe;
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    let measurement = camera.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(cookie, 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    if drop_measurement {
        drop(measurement);
    } else {
        measurement.stop().unwrap();
    }

    assert!(sink_dropped.load(Ordering::SeqCst));
    assert!(stop_entered.load(Ordering::SeqCst));
    invoke_mono(cookie, 2, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        mock.logs().iter().filter(|entry| **entry == "stop").count(),
        1
    );

    camera.close().unwrap();
    runtime.shutdown().unwrap();
}

fn assert_exception_close_order(close_result: DriverResult<()>) {
    let close_succeeds = close_result.is_ok();
    let mock = MockDriver::new();
    mock.push_close(close_result);
    let close_entered = Arc::new(AtomicBool::new(false));
    mock.set_close_entered(Arc::clone(&close_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let sink_dropped = Arc::new(AtomicBool::new(false));
    let calls = Arc::new(AtomicUsize::new(0));
    let probe = DropBeforeDriverCall {
        driver_entered: Arc::clone(&close_entered),
        sink_dropped: Arc::clone(&sink_dropped),
    };
    let sink_calls = Arc::clone(&calls);
    let sink: ExceptionCallbackSink = Arc::new(move |_| {
        let _keep_probe_alive = &probe;
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    camera.register_exception_callback(sink).unwrap();
    let cookie = only_cookie(mock.exception_callback_cookies());
    invoke_disconnect(cookie);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let close = camera.close();
    assert_eq!(close.is_ok(), close_succeeds);
    assert!(sink_dropped.load(Ordering::SeqCst));
    assert!(close_entered.load(Ordering::SeqCst));
    invoke_disconnect(cookie);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    if close_succeeds {
        runtime.shutdown().unwrap();
    } else {
        assert!(matches!(
            runtime.shutdown(),
            Err(Error::UnclosedDevices {
                live_handles: 0,
                teardown_uncertain: true,
            })
        ));
    }
}

struct DropBeforeDriverCall {
    driver_entered: Arc<AtomicBool>,
    sink_dropped: Arc<AtomicBool>,
}

struct PanicOnDrop;

impl Drop for PanicOnDrop {
    fn drop(&mut self) {
        panic!("panic payload destructor must never run in the trampoline");
    }
}

impl Drop for DropBeforeDriverCall {
    fn drop(&mut self) {
        assert!(
            !self.driver_entered.load(Ordering::SeqCst),
            "callback sink was retained until after the native stop/close call began"
        );
        self.sink_dropped.store(true, Ordering::SeqCst);
    }
}

fn only_cookie(
    mut cookies: Vec<crate::callback::CallbackCookie>,
) -> crate::callback::CallbackCookie {
    assert_eq!(cookies.len(), 1);
    cookies.pop().unwrap()
}

fn invoke_mono(cookie: crate::callback::CallbackCookie, frame_number: u32, value: u8) {
    let (mut image, data) = mono_descriptor(frame_number, value);
    invoke_image_for_test(cookie, &mut image);
    drop(data);
}

fn mono_descriptor(frame_number: u32, value: u8) -> (MV3D_LP_IMAGE_DATA, Vec<u8>) {
    let mut data = vec![value];
    let mut image = crate::ffi::zeroed_image();
    image.enImageType = ImageType_Mono8;
    image.nWidth = 1;
    image.nHeight = 1;
    image.pData = data.as_mut_ptr();
    image.nDataLen = data.len() as u32;
    image.nFrameNum = frame_number;
    image.bValid = 1;
    (image, data)
}

fn invoke_disconnect(cookie: crate::callback::CallbackCookie) {
    let mut exception = exception_descriptor(DevExceptionType_Disconnect);
    copy_exception_description(&mut exception, b"offline\0");
    invoke_exception_for_test(cookie, &mut exception);
}

fn exception_descriptor(kind: i32) -> MV3D_LP_EXCEPTION_INFO {
    MV3D_LP_EXCEPTION_INFO {
        enExceptionType: kind,
        chExceptionDesc: [0; MV3D_LP_MAX_STRING_LENGTH],
        nReserved: [0; 4],
    }
}

fn copy_exception_description(exception: &mut MV3D_LP_EXCEPTION_INFO, description: &[u8]) {
    for (destination, source) in exception.chExceptionDesc.iter_mut().zip(description) {
        *destination = *source as i8;
    }
}

fn release_callback(release: &(Mutex<bool>, Condvar)) {
    let (released, wake) = release;
    *released
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
    wake.notify_all();
}
