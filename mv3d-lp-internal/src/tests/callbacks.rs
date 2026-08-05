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
use crate::opened_device::DeviceState;

use super::mock_driver::{MockDriver, active_runtime};

// 验证图像 callback 在返回前复制全部 payload 与元数据，防止引用 SDK 临时缓冲区。
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

// 验证异常 callback 限制描述长度并保留未知类型，防止越界读取和事件值丢失。
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

// 验证并发 callback 按 cookie 精确投递一次，防止不同设备注册之间串流。
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

// 验证注册注销先拒绝新投递再等待在途 callback，防止 sink 释放期间继续访问。
#[test]
fn registration_drop_rejects_new_admissions_and_waits_for_in_flight_callback() {
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
    let registration = CallbackRegistration::image(sink).unwrap();
    let cookie = registration.cookie();

    let callback = thread::spawn(move || invoke_mono(cookie, 1, 1));
    entered_receiver.recv().unwrap();

    let (started_sender, started_receiver) = mpsc::channel();
    let (done_sender, done_receiver) = mpsc::channel();
    let retire = thread::spawn(move || {
        started_sender.send(()).unwrap();
        drop(registration);
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
    retire.join().unwrap();
    done_receiver.recv().unwrap();
    let before = calls.load(Ordering::SeqCst);
    invoke_mono(cookie, 3, 3);
    assert_eq!(calls.load(Ordering::SeqCst), before);
}

// 验证队列满仅丢当前事件而接收端断开会关闭 slot，防止 callback 阻塞或空转。
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

// 验证畸形 descriptor 只记失败且有效流继续工作，防止单帧错误终止 callback。
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

// 验证零值和未知 cookie 在读取 payload 前被拒绝，防止无归属指针被解引用。
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

// 验证 sink panic 被 trampoline 捕获并关闭注册，防止 unwind 穿过 C ABI。
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

// 验证 panic payload 析构再次 panic 时仍受边界约束，防止双重 unwind 穿过 C ABI。
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

// 验证图像 callback 注册失败后立即注销 cookie，防止迟到投递进入失败的 sink。
#[test]
fn failed_image_registration_retires_the_cookie_before_late_delivery() {
    let mock = MockDriver::new();
    mock.push_register_image_callback(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    assert!(matches!(
        device.start_callback(Arc::clone(&sink)),
        Err(Error::Sdk { .. })
    ));
    let cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(cookie, 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(device.state(), DeviceState::Open);

    device.start_callback(sink).unwrap();
    let cookies = mock.image_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_mono(cookies[0], 2, 2);
    invoke_mono(cookies[1], 3, 3);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    device.stop().unwrap();

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证异常 callback 注册失败后立即注销 cookie，防止迟到事件访问失败注册。
#[test]
fn failed_exception_registration_retires_the_cookie_before_late_delivery() {
    let mock = MockDriver::new();
    mock.push_register_exception_callback(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: ExceptionCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    assert!(matches!(
        device.register_exception_callback(Arc::clone(&sink)),
        Err(Error::Sdk { .. })
    ));
    let cookie = only_cookie(mock.exception_callback_cookies());
    invoke_disconnect(cookie);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(device.state(), DeviceState::Open);

    device.register_exception_callback(sink).unwrap();
    let cookies = mock.exception_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_disconnect(cookies[0]);
    invoke_disconnect(cookies[1]);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证 callback 测量停止后使用新 cookie 重启，防止旧注册状态污染下一次采集。
#[test]
fn callback_device_can_restart_with_a_fresh_cookie_after_stop() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let first_calls = Arc::new(AtomicUsize::new(0));
    let first_sink_calls = Arc::clone(&first_calls);
    let first_sink: FrameCallbackSink = Arc::new(move |_| {
        first_sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    device.start_callback(first_sink).unwrap();
    assert!(device.image_callback_stats().is_some());
    let first_cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(first_cookie, 1, 1);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    device.stop().unwrap();
    assert_eq!(device.state(), DeviceState::Open);
    assert!(device.image_callback_stats().is_none());
    invoke_mono(first_cookie, 2, 2);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);

    let second_calls = Arc::new(AtomicUsize::new(0));
    let second_sink_calls = Arc::clone(&second_calls);
    let second_sink: FrameCallbackSink = Arc::new(move |_| {
        second_sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    device.start_callback(second_sink).unwrap();
    let cookies = mock.image_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_mono(cookies[0], 3, 3);
    invoke_mono(cookies[1], 4, 4);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(Ordering::SeqCst), 1);
    device.stop().unwrap();
    assert_eq!(device.state(), DeviceState::Open);

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证重复图像注册直接透传 SDK 错误，防止 Rust 状态误报注册成功。
#[test]
fn repeated_image_registration_forwards_the_native_error() {
    let mock = MockDriver::new();
    mock.push_register_image_callback(Ok(()));
    mock.push_register_image_callback(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let first_sink: FrameCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
    device.start_callback(first_sink).unwrap();
    device.stop().unwrap();

    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let second_sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    assert!(matches!(
        device.start_callback(second_sink),
        Err(Error::Sdk { .. })
    ));
    assert_eq!(device.state(), DeviceState::Open);
    let cookies = mock.image_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_mono(cookies[1], 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证 callback 启动失败会注销 cookie 且允许重试，防止失败注册永久占用 slot。
#[test]
fn failed_callback_start_retires_its_cookie_and_can_be_retried() {
    let mock = MockDriver::new();
    mock.push_start(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    assert!(matches!(
        device.start_callback(Arc::clone(&sink)),
        Err(Error::Sdk { .. })
    ));
    assert_eq!(device.state(), DeviceState::Open);
    let first_cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(first_cookie, 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);

    device.start_callback(sink).unwrap();
    let cookies = mock.image_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_mono(cookies[0], 2, 2);
    invoke_mono(cookies[1], 3, 3);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    device.stop().unwrap();

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证 callback stop 失败仍先注销 cookie，随后 close 重试 stop，防止 Faulted 设备继续投递。
#[test]
fn failed_callback_stop_retires_its_cookie_and_close_retries_stop() {
    let mock = MockDriver::new();
    mock.push_stop(Err(DriverError::Status(0x8006_0003_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });

    device.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(cookie, 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    assert!(matches!(device.stop(), Err(Error::Sdk { .. })));
    assert_eq!(device.state(), DeviceState::Faulted);
    assert!(device.image_callback_stats().is_none());
    invoke_mono(cookie, 2, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    device.close().unwrap();
    assert_eq!(
        mock.logs().iter().filter(|entry| **entry == "stop").count(),
        2
    );
    runtime.shutdown().unwrap();
}

// 验证异常 callback 替换成功后注销旧 cookie，防止新旧 sink 同时接收事件。
#[test]
fn exception_callback_registration_replaces_and_retires_the_previous_cookie() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let first_calls = Arc::new(AtomicUsize::new(0));
    let first_sink_calls = Arc::clone(&first_calls);
    let first_sink: ExceptionCallbackSink = Arc::new(move |_| {
        first_sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    device.register_exception_callback(first_sink).unwrap();
    let first_cookie = only_cookie(mock.exception_callback_cookies());
    invoke_disconnect(first_cookie);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);

    let second_calls = Arc::new(AtomicUsize::new(0));
    let second_sink_calls = Arc::clone(&second_calls);
    let second_sink: ExceptionCallbackSink = Arc::new(move |_| {
        second_sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    device.register_exception_callback(second_sink).unwrap();
    let cookies = mock.exception_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_disconnect(cookies[0]);
    invoke_disconnect(cookies[1]);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(Ordering::SeqCst), 1);

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证异常 callback 替换失败时旧注册继续有效，防止错误清除工作中的 sink。
#[test]
fn failed_exception_callback_replacement_keeps_the_previous_cookie_active() {
    let mock = MockDriver::new();
    mock.push_register_exception_callback(Ok(()));
    mock.push_register_exception_callback(Err(DriverError::Status(0x8006_0005_u32 as i32)));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let first_calls = Arc::new(AtomicUsize::new(0));
    let first_sink_calls = Arc::clone(&first_calls);
    let first_sink: ExceptionCallbackSink = Arc::new(move |_| {
        first_sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    device.register_exception_callback(first_sink).unwrap();

    let second_calls = Arc::new(AtomicUsize::new(0));
    let second_sink_calls = Arc::clone(&second_calls);
    let second_sink: ExceptionCallbackSink = Arc::new(move |_| {
        second_sink_calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Delivered
    });
    assert!(matches!(
        device.register_exception_callback(second_sink),
        Err(Error::Sdk { .. })
    ));
    assert_eq!(device.state(), DeviceState::Open);
    let cookies = mock.exception_callback_cookies();
    assert_eq!(cookies.len(), 2);
    assert_ne!(cookies[0], cookies[1]);
    invoke_disconnect(cookies[0]);
    invoke_disconnect(cookies[1]);
    assert_eq!(first_calls.load(Ordering::SeqCst), 1);
    assert_eq!(second_calls.load(Ordering::SeqCst), 0);

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证停用异常投递先拒绝新事件并排空在途 callback，防止 sink 释放期间继续访问。
#[test]
fn disabling_exception_delivery_drains_in_flight_and_rejects_late_cookie() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let (entered_sender, entered_receiver) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let sink_release = Arc::clone(&release);
    let sink: ExceptionCallbackSink = Arc::new(move |_| {
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
    device.register_exception_callback(sink).unwrap();
    let cookie = only_cookie(mock.exception_callback_cookies());

    let mut device = thread::scope(|scope| {
        let callback = scope.spawn(move || invoke_disconnect(cookie));
        entered_receiver.recv().unwrap();

        let (started_sender, started_receiver) = mpsc::channel();
        let (done_sender, done_receiver) = mpsc::channel();
        let disable = scope.spawn(move || {
            started_sender.send(()).unwrap();
            device.disable_exception_delivery();
            done_sender.send(()).unwrap();
            device
        });
        started_receiver.recv().unwrap();

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            let before = calls.load(Ordering::SeqCst);
            invoke_disconnect(cookie);
            if calls.load(Ordering::SeqCst) == before {
                break;
            }
            if Instant::now() >= deadline {
                release_callback(&release);
                panic!("exception delivery was not revoked");
            }
            thread::yield_now();
        }
        assert_eq!(done_receiver.try_recv(), Err(mpsc::TryRecvError::Empty));

        release_callback(&release);
        callback.join().unwrap();
        let device = disable.join().unwrap();
        done_receiver.recv().unwrap();
        device
    });

    assert!(device.exception_callback_stats().is_none());
    device.disable_exception_delivery();
    let calls_after_disable = calls.load(Ordering::SeqCst);
    invoke_disconnect(cookie);
    assert_eq!(calls.load(Ordering::SeqCst), calls_after_disable);

    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证显式 stop 与 active Device Drop 都先停用 callback，防止停止采集期间接纳新事件。
#[test]
fn callback_device_deactivates_before_explicit_and_drop_stop() {
    assert_callback_stop_order(false);
    assert_callback_stop_order(true);
}

// 验证关闭 active callback Device 会注销 cookie，防止 handle 关闭后继续投递。
#[test]
fn device_close_retires_the_cookie_of_active_callback_acquisition() {
    let mock = MockDriver::new();
    let stop_entered = Arc::new(AtomicBool::new(false));
    mock.set_stop_entered(Arc::clone(&stop_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
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
    device.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());

    assert!(matches!(
        device.clear_buffer(),
        Err(Error::InvalidState { .. })
    ));
    assert!(!mock.logs().contains(&"clear_buffer"));
    device.close().unwrap();
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

// 验证异常注册在 close 前停用且可承受迟到 callback，防止关闭后访问已释放 sink。
#[test]
fn exception_registration_deactivates_before_close_and_survives_late_callbacks() {
    assert_exception_close_order(Ok(()));
    assert_exception_close_order(Err(DriverError::Status(0x8006_0000_u32 as i32)));
}

fn assert_callback_stop_order(drop_device: bool) {
    let mock = MockDriver::new();
    let stop_entered = Arc::new(AtomicBool::new(false));
    mock.set_stop_entered(Arc::clone(&stop_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
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
    device.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());
    invoke_mono(cookie, 1, 1);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    if drop_device {
        drop(device);
    } else {
        device.stop().unwrap();
        assert_eq!(device.state(), DeviceState::Open);
        assert!(device.image_callback_stats().is_none());
        device.close().unwrap();
    }

    assert!(sink_dropped.load(Ordering::SeqCst));
    assert!(stop_entered.load(Ordering::SeqCst));
    invoke_mono(cookie, 2, 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        mock.logs().iter().filter(|entry| **entry == "stop").count(),
        1
    );
    runtime.shutdown().unwrap();
}

fn assert_exception_close_order(close_result: DriverResult<()>) {
    let close_succeeds = close_result.is_ok();
    let mock = MockDriver::new();
    mock.push_close(close_result);
    let close_entered = Arc::new(AtomicBool::new(false));
    mock.set_close_entered(Arc::clone(&close_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
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
    device.register_exception_callback(sink).unwrap();
    let cookie = only_cookie(mock.exception_callback_cookies());
    invoke_disconnect(cookie);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let close = device.close();
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
