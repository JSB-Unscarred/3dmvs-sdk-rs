use std::net::Ipv4Addr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::bindings::ImageType_Mono8;
use crate::callback::{CallbackCookie, CallbackDelivery, FrameCallbackSink, invoke_image_for_test};
use crate::frame::{FrameRecord, ImageTypeRecord};
use crate::opened_device::DeviceState;
use crate::parameter::ParameterRecord;

use super::mock_driver::{FfiOp, MockDriver, active_runtime};

#[test]
fn moved_device_supports_a_complete_pull_acquisition() {
    let mock = MockDriver::new();
    mock.push_get_parameter(Ok(ParameterRecord::Bool(true)));
    mock.push_get_image(Ok(frame(vec![1, 2, 3])));
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    thread::scope(|scope| {
        scope
            .spawn(move || {
                let mut device = device;
                assert_eq!(
                    device.get_parameter(b"AcquisitionEnabled").unwrap(),
                    ParameterRecord::Bool(true)
                );

                let mut measurement = device.start().unwrap();
                assert_eq!(measurement.get_image(37).unwrap().data, [1, 2, 3]);
                measurement.stop().unwrap();
                device.close().unwrap();
            })
            .join()
            .unwrap();
    });

    runtime.shutdown().unwrap();
    assert_eq!(mock.image_timeouts(), [37]);
    assert_eq!(
        mock.logs(),
        [
            "version",
            "initialize",
            "open_by_ip",
            "get_parameter",
            "start",
            "get_image",
            "stop",
            "close",
            "finalize",
        ]
    );
}

#[test]
fn moved_device_cleans_up_on_target_thread_panic() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let outcome = catch_unwind(AssertUnwindSafe(|| {
        thread::scope(|scope| {
            scope
                .spawn(move || {
                    let mut device = device;
                    let _measurement = device.start().unwrap();
                    panic!("intentional target-thread panic");
                })
                .join()
                .unwrap();
        });
    }));
    assert!(outcome.is_err());
    assert_eq!(
        &mock.logs()[mock.logs().len() - 3..],
        ["start", "stop", "close"]
    );

    runtime.shutdown().unwrap();
}

#[test]
fn moved_measurement_supports_explicit_stop_and_drop() {
    for explicit_stop in [true, false] {
        let mock = MockDriver::new();
        let (runtime, _) = active_runtime(&mock);
        let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

        thread::scope(|scope| {
            let measurement = device.start().unwrap();
            scope
                .spawn(move || {
                    if explicit_stop {
                        measurement.stop().unwrap();
                    } else {
                        drop(measurement);
                    }
                })
                .join()
                .unwrap();
        });

        assert_eq!(device.state(), DeviceState::Open);
        device.close().unwrap();
        runtime.shutdown().unwrap();
        assert_eq!(
            mock.logs()
                .iter()
                .filter(|operation| **operation == "stop")
                .count(),
            1
        );
    }
}

#[test]
fn moved_callback_measurement_drains_an_in_flight_callback_before_stop() {
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
    let measurement = device.start_callback(sink).unwrap();
    let cookie = only_cookie(mock.image_callback_cookies());
    let stop_started = Arc::new(Signal::default());

    thread::scope(|scope| {
        let callback = scope.spawn(move || invoke_mono(cookie, 1, 7));
        callback_entered.wait();

        let worker_started = Arc::clone(&stop_started);
        let stopper = scope.spawn(move || {
            worker_started.signal();
            measurement.stop().unwrap();
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
        stopper.join().unwrap();
    });

    assert!(stop_entered.load(Ordering::SeqCst));
    let calls_after_stop = callback_calls.load(Ordering::SeqCst);
    invoke_mono(cookie, 3, 11);
    assert_eq!(callback_calls.load(Ordering::SeqCst), calls_after_stop);
    device.close().unwrap();
    runtime.shutdown().unwrap();
}

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

fn frame(data: Vec<u8>) -> FrameRecord {
    FrameRecord {
        image_type: ImageTypeRecord::from_bits(0x0108_0001),
        width: u32::try_from(data.len()).unwrap(),
        height: 1,
        data,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 7,
        device_timestamp: 11,
        valid: true,
        x_scale: 1.0,
        y_scale: 2.0,
        z_scale: 3.0,
        x_offset: 4,
        y_offset: 5,
        z_offset: 6,
    }
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
