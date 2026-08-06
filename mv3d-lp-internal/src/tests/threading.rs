use std::net::Ipv4Addr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::bindings::ImageType_Mono8;
use crate::callback::{CallbackCookie, CallbackDelivery, FrameCallbackSink, invoke_image_for_test};
use crate::parameter::ParameterRecord;

use super::mock_driver::{FfiOp, MockDriver, active_runtime};

// 验证 Device 与 session 关闭可在 Runtime token 释放后由普通线程接管。
#[test]
fn moved_device_and_session_outlive_the_runtime_token() {
    let mock = MockDriver::new();
    let (runtime, gate) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    drop(runtime);

    let worker_mock = mock.clone();
    thread::spawn(move || {
        let mut device = device;
        device.clear_buffer().unwrap();
        device.close().unwrap();
        crate::runtime::Runtime::initialize_with(Box::new(worker_mock), gate)
            .unwrap()
            .shutdown()
            .unwrap();
    })
    .join()
    .unwrap();

    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "open_by_ip",
            "clear_buffer",
            "close",
            "finalize",
        ]
    );
}

// 验证移动 active Device 停止前排空在途 callback，防止 sink 仍执行时停止 SDK。
#[test]
fn moved_callback_device_drains_an_in_flight_callback_before_stop() {
    let mock = MockDriver::new();
    let stop_entered = Arc::new(AtomicBool::new(false));
    mock.set_stop_entered(Arc::clone(&stop_entered));
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let callback_entered = Arc::new(Signal::default());
    let release_callback = Arc::new(Signal::default());
    let callback_calls = Arc::new(AtomicUsize::new(0));
    let sink_entered = Arc::clone(&callback_entered);
    let sink_release = Arc::clone(&release_callback);
    let sink_calls = Arc::clone(&callback_calls);
    let sink: FrameCallbackSink = Arc::new(move |_| {
        if sink_calls.fetch_add(1, Ordering::SeqCst) == 0 {
            sink_entered.signal();
            sink_release.wait();
        }
        CallbackDelivery::Delivered
    });
    device.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());
    let stop_started = Arc::new(Signal::default());

    let device = thread::scope(|scope| {
        let callback = scope.spawn(move || invoke_mono(cookie, 1, 7));
        callback_entered.wait();

        let worker_started = Arc::clone(&stop_started);
        let stopper = scope.spawn(move || {
            let mut device = device;
            worker_started.signal();
            device.stop().unwrap();
            device
        });
        stop_started.wait();

        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            let calls_before = callback_calls.load(Ordering::SeqCst);
            invoke_mono(cookie, 2, 9);
            if callback_calls.load(Ordering::SeqCst) == calls_before {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "callback registration was not revoked before stop"
            );
            thread::yield_now();
        }
        assert!(!stop_entered.load(Ordering::SeqCst));

        release_callback.signal();
        callback.join().unwrap();
        stopper.join().unwrap()
    });

    assert!(stop_entered.load(Ordering::SeqCst));
    let calls_after_stop = callback_calls.load(Ordering::SeqCst);
    invoke_mono(cookie, 3, 11);
    assert_eq!(callback_calls.load(Ordering::SeqCst), calls_after_stop);
    device.close().unwrap();
    runtime.shutdown().unwrap();
}

// 验证不同设备的普通调用可并行，防止 runtime 锁将独立 handle 全局串行化。
#[test]
fn ordinary_calls_on_distinct_devices_can_overlap() {
    let mock = MockDriver::new();
    mock.push_get_parameter(Ok(ParameterRecord::Bool(true)));

    let parameter_entered = Arc::new(Signal::default());
    let execute_entered = Arc::new(Signal::default());
    let overlap_observed = Arc::new(AtomicBool::new(false));
    let hook_parameter_entered = Arc::clone(&parameter_entered);
    let hook_execute_entered = Arc::clone(&execute_entered);
    let hook_overlap = Arc::clone(&overlap_observed);
    mock.hook_next_calls(
        FfiOp::GetParameter,
        1,
        Arc::new(move || {
            hook_parameter_entered.signal();
            if hook_execute_entered.wait_timeout(Duration::from_secs(5)) {
                hook_overlap.store(true, Ordering::SeqCst);
            }
        }),
    );
    let hook_execute_entered = Arc::clone(&execute_entered);
    mock.hook_next_calls(
        FfiOp::Execute,
        1,
        Arc::new(move || hook_execute_entered.signal()),
    );

    let (runtime, _) = active_runtime(&mock);
    let first = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let second = runtime.open_by_serial(b"SECOND").unwrap();

    thread::scope(|scope| {
        let first_worker = scope.spawn(move || {
            let mut first = first;
            assert_eq!(
                first.get_parameter(b"AcquisitionEnabled").unwrap(),
                ParameterRecord::Bool(true)
            );
            first.close().unwrap();
        });
        parameter_entered.wait();
        let second_worker = scope.spawn(move || {
            let mut second = second;
            second.execute(b"DeviceReset").unwrap();
            second.close().unwrap();
        });

        first_worker.join().unwrap();
        second_worker.join().unwrap();
    });

    assert!(overlap_observed.load(Ordering::SeqCst));
    assert!(mock.maximum_in_flight() >= 2);
    assert_eq!(mock.in_flight(), 0);
    let mut closed = mock.closed_handles();
    closed.sort_unstable();
    assert_eq!(closed, [1, 2]);
    runtime.shutdown().unwrap();
}

fn only_cookie(mut cookies: Vec<CallbackCookie>) -> CallbackCookie {
    assert_eq!(cookies.len(), 1);
    cookies.pop().unwrap()
}

fn invoke_mono(cookie: CallbackCookie, frame_number: u32, value: u8) {
    let mut data = vec![value];
    let mut image = crate::ffi::zeroed_image();
    image.enImageType = ImageType_Mono8;
    image.nWidth = 1;
    image.nHeight = 1;
    image.pData = data.as_mut_ptr();
    image.nDataLen = data.len() as u32;
    image.nFrameNum = frame_number;
    image.bValid = 1;
    invoke_image_for_test(cookie, &mut image);
}

#[derive(Default)]
struct Signal {
    signaled: Mutex<bool>,
    wake: Condvar,
}

impl Signal {
    fn signal(&self) {
        *self
            .signaled
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = true;
        self.wake.notify_all();
    }

    fn wait(&self) {
        let signaled = self
            .signaled
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        drop(
            self.wake
                .wait_while(signaled, |signaled| !*signaled)
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
        );
    }

    fn wait_timeout(&self, timeout: Duration) -> bool {
        let signaled = self
            .signaled
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (signaled, _) = self
            .wake
            .wait_timeout_while(signaled, timeout, |signaled| !*signaled)
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        *signaled
    }
}
