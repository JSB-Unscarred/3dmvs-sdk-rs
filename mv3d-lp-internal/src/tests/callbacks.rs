use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use crate::bindings::{ImageType_Mono8, MV3D_LP_IMAGE_DATA};
use crate::callback::{
    CallbackDelivery, CallbackRegistration, FrameCallbackSink, invoke_image_for_test,
};
use crate::frame::FrameRecord;

// 验证 callback 返回前复制全部 payload，后续修改 SDK buffer 不影响结果。
#[test]
fn image_callback_copies_all_payloads() {
    let (sender, receiver) = mpsc::channel::<FrameRecord>();
    let sink: FrameCallbackSink = Arc::new(move |frame| {
        sender.send(frame).unwrap();
        CallbackDelivery::Delivered
    });
    let registration = CallbackRegistration::image(sink);
    let mut data = vec![1, 2, 3, 4];
    let mut intensity = vec![5, 6, 7, 8];
    let mut exposure = vec![1_000_i64, 2_000];
    let mut image = image_descriptor(&mut data, 2, 2);
    image.pIntensityData = intensity.as_mut_ptr();
    image.nIntensityDataLen = intensity.len() as u32;
    image.pExposureTimeStamp = exposure.as_mut_ptr();

    invoke_image_for_test(registration.cookie(), &mut image);
    data.fill(0);
    intensity.fill(0);
    exposure.fill(0);

    let frame = receiver.recv().unwrap();
    assert_eq!(frame.data, [1, 2, 3, 4]);
    assert_eq!(frame.intensity_data, Some(vec![5, 6, 7, 8]));
    assert_eq!(frame.exposure_timestamps, Some(vec![1_000, 2_000]));
    assert_eq!(registration.stats().delivered, 1);
}

// 验证 Full 仅丢当前帧，Disconnected 则关闭后续投递。
#[test]
fn bounded_delivery_distinguishes_full_and_disconnected() {
    let full_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&full_calls);
    let full = CallbackRegistration::image(Arc::new(move |_| {
        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
            CallbackDelivery::Full
        } else {
            CallbackDelivery::Delivered
        }
    }));
    invoke_mono(full.cookie(), 1);
    invoke_mono(full.cookie(), 2);
    assert_eq!(full.stats().dropped_full, 1);
    assert_eq!(full.stats().delivered, 1);

    let disconnected_calls = Arc::new(AtomicUsize::new(0));
    let calls = Arc::clone(&disconnected_calls);
    let disconnected = CallbackRegistration::image(Arc::new(move |_| {
        calls.fetch_add(1, Ordering::SeqCst);
        CallbackDelivery::Disconnected
    }));
    invoke_mono(disconnected.cookie(), 1);
    invoke_mono(disconnected.cookie(), 2);
    assert_eq!(disconnected_calls.load(Ordering::SeqCst), 1);
    assert!(!disconnected.stats().accepting);
}

// 验证注销先拒绝新 callback，再等待已经进入的 callback 返回。
#[test]
fn registration_drop_rejects_late_cookie_and_waits_for_in_flight() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let (entered_sender, entered_receiver) = mpsc::channel();
    let release = Arc::new((Mutex::new(false), Condvar::new()));
    let sink_release = Arc::clone(&release);
    let registration = CallbackRegistration::image(Arc::new(move |_| {
        if sink_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            entered_sender.send(()).unwrap();
            let (released, wake) = &*sink_release;
            let mut released = released.lock().unwrap();
            while !*released {
                released = wake.wait(released).unwrap();
            }
        }
        CallbackDelivery::Delivered
    }));
    let cookie = registration.cookie();

    let callback = thread::spawn(move || invoke_mono(cookie, 1));
    entered_receiver.recv().unwrap();
    let (done_sender, done_receiver) = mpsc::channel();
    let retire = thread::spawn(move || {
        drop(registration);
        done_sender.send(()).unwrap();
    });

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let before = calls.load(Ordering::SeqCst);
        invoke_mono(cookie, 2);
        if calls.load(Ordering::SeqCst) == before {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "callback registration did not retire"
        );
    }
    assert!(done_receiver.try_recv().is_err());
    let retired_count = calls.load(Ordering::SeqCst);
    release_callback(&release);
    callback.join().unwrap();
    retire.join().unwrap();
    done_receiver.recv().unwrap();
    invoke_mono(cookie, 3);
    assert_eq!(calls.load(Ordering::SeqCst), retired_count);
}

// 验证未知 cookie 在读取恶意 payload pointer 前直接拒绝。
#[test]
fn unknown_cookie_is_rejected_before_payload_access() {
    let mut hostile = crate::ffi::zeroed_image();
    hostile.enImageType = ImageType_Mono8;
    hostile.nWidth = 1;
    hostile.nHeight = 1;
    hostile.pData = ptr::without_provenance_mut(1);
    hostile.nDataLen = 1;

    // SAFETY: descriptor 可读，payload 故意无效；未知 cookie 必须先于 payload 被拒绝。
    unsafe {
        crate::callback::image_trampoline(&mut hostile, ptr::null_mut());
        crate::callback::image_trampoline(&mut hostile, ptr::without_provenance_mut(usize::MAX));
    }
}

// 验证用户 sink panic 不跨越 C ABI，并关闭该注册。
#[test]
fn sink_panic_is_contained_and_fails_closed() {
    let calls = Arc::new(AtomicUsize::new(0));
    let sink_calls = Arc::clone(&calls);
    let registration = CallbackRegistration::image(Arc::new(move |_| {
        sink_calls.fetch_add(1, Ordering::SeqCst);
        panic!("intentional callback panic");
    }));

    assert!(catch_unwind(AssertUnwindSafe(|| invoke_mono(registration.cookie(), 1))).is_ok());
    invoke_mono(registration.cookie(), 2);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(registration.stats().panics, 1);
    assert!(!registration.stats().accepting);
}

fn invoke_mono(cookie: crate::callback::CallbackCookie, frame_number: u32) {
    let mut data = vec![frame_number as u8];
    let mut image = image_descriptor(&mut data, 1, 1);
    image.nFrameNum = frame_number;
    invoke_image_for_test(cookie, &mut image);
}

fn image_descriptor(data: &mut [u8], width: u32, height: u32) -> MV3D_LP_IMAGE_DATA {
    let mut image = crate::ffi::zeroed_image();
    image.enImageType = ImageType_Mono8;
    image.nWidth = width;
    image.nHeight = height;
    image.pData = data.as_mut_ptr();
    image.nDataLen = data.len() as u32;
    image.bValid = 1;
    image
}

fn release_callback(release: &(Mutex<bool>, Condvar)) {
    let (released, wake) = release;
    *released.lock().unwrap() = true;
    wake.notify_all();
}
