#![cfg(all(target_os = "windows", target_arch = "x86_64", target_env = "msvc"))]

use std::env;
use std::fmt::Display;
use std::fs;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use mv3d_lp_internal::{
    CallbackDelivery, Device, DeviceState, Error, ExceptionCallbackSink, FileTransfer,
    FrameCallbackSink, Measurement, Runtime,
};

const EXPECTED_VERSION: &[u8] = b"1.3.3.3";
const NO_DATA_STATUS: u32 = 0x8006_0006;
const GET_IMAGE_SLICE_MS: u32 = 500;
const GET_IMAGE_DEADLINE: Duration = Duration::from_secs(10);
const CALLBACK_DEADLINE: Duration = Duration::from_secs(15);
const FILE_POLL_INTERVAL: Duration = Duration::from_millis(50);
const FILE_DEADLINE: Duration = Duration::from_secs(60);
const PROCESS_DEADLINE: Duration = Duration::from_secs(5 * 60);

const SERIAL_ENV: &str = "MV3D_LP_TEST_SERIAL";
const DEVICE_FILE_ENV: &str = "MV3D_LP_TEST_DEVICE_READ_FILE";
const SCRATCH_ENV: &str = "MV3D_LP_TEST_LOCAL_SCRATCH_DIR";

type TestResult<T = ()> = Result<T, String>;

#[test]
#[ignore]
fn single_runtime_cross_thread_contract() {
    let _deadline = ProcessDeadline::start(PROCESS_DEADLINE);
    if let Err(error) = run_contract() {
        panic!("LPSDK native thread contract failed: {error}");
    }
}

fn run_contract() -> TestResult {
    let serial = required_ascii_env(SERIAL_ENV, Some(16))?;
    let device_file = required_ascii_env(DEVICE_FILE_ENV, None)?;
    let mut scratch = ScratchDirectory::from_env()?;

    let runtime = sdk(
        "strictly initialize the LPSDK runtime",
        Runtime::initialize_strict(),
    )?;
    let version_matches = runtime.version_bytes() == EXPECTED_VERSION;
    let contract = if version_matches {
        run_scenarios(&runtime, &serial, &device_file, &mut scratch)
    } else {
        Err(String::from(
            "MV3D_LP_GetVersion did not return the exact audited version 1.3.3.3",
        ))
    };

    let shutdown = sdk("finalize the LPSDK runtime on thread A", runtime.shutdown());
    let scratch_cleanup = scratch.cleanup();

    let contract = merge_results(contract, shutdown, "Runtime shutdown");
    merge_results(contract, scratch_cleanup, "scratch cleanup")
}

fn run_scenarios(
    runtime: &Runtime,
    serial: &[u8],
    device_file: &[u8],
    scratch: &mut ScratchDirectory,
) -> TestResult {
    // Version must be checked before discovery, open, or any other device operation.
    assert_eq!(runtime.version_bytes(), EXPECTED_VERSION);
    let (model, redacted_id) = selected_device_metadata(runtime, serial)?;
    println!(
        "native acceptance: target=x86_64-pc-windows-msvc dll=1.3.3.3 model={model} device={redacted_id} thread_a={:?}",
        thread::current().id()
    );

    scenario("1 device control and close/drop on thread B", || {
        device_control_and_drop(runtime, serial)
    })?;
    scenario("2 acquisition guards stop/drop on thread B", || {
        acquisition_guard_handoff(runtime, serial)
    })?;
    scenario("3 exception callback closes on thread B", || {
        exception_callback_handoff(runtime, serial)
    })?;

    let started_on_b = scratch.target("started-on-thread-b.bin")?;
    scenario("4 FileAccess starts and completes on thread B", || {
        file_access_started_on_b(runtime, serial, device_file, &started_on_b)
    })?;
    require_regular_file(&started_on_b.path, "thread-B FileAccess output")?;

    let started_on_a = scratch.target("started-on-thread-a.bin")?;
    scenario("5 active FileAccess handoff completes on thread B", || {
        active_transfer_handoff(runtime, serial, device_file, &started_on_a)
    })?;
    require_regular_file(&started_on_a.path, "thread-A FileAccess output")?;

    let resumed_after_drop = scratch.target("resumed-after-drop.bin")?;
    scenario(
        "6 FileAccess guard starts/drops on thread B and resumes on A",
        || dropped_transfer_resumes(runtime, serial, device_file, &resumed_after_drop),
    )?;
    require_regular_file(&resumed_after_drop.path, "resumed FileAccess output")?;

    let closed_after_drop = scratch.target("closed-after-guard-drop.bin")?;
    let device_dropped_after_guard = scratch.target("device-dropped-after-guard.bin")?;
    scenario(
        "7 Device closes/drops immediately after FileAccess guard Drop on thread B",
        || {
            immediate_device_cleanup_after_guard_drop(
                runtime,
                serial,
                device_file,
                &closed_after_drop,
                &device_dropped_after_guard,
            )
        },
    )?;

    println!("native acceptance: all ordered scenarios passed");
    Ok(())
}

fn device_control_and_drop(runtime: &Runtime, serial: &[u8]) -> TestResult {
    let device = open_device(runtime, serial)?;
    on_thread_b("native-device-close", move || {
        let mut device = device;
        sdk(
            "query ExposureTime on thread B",
            device.get_parameter(b"ExposureTime"),
        )?;
        let mut measurement = sdk("start pull acquisition on thread B", device.start())?;
        let image = wait_for_image(&mut measurement);
        let stop = sdk("stop pull acquisition on thread B", measurement.stop());
        let acquisition = merge_results(image, stop, "pull acquisition cleanup");
        let close = sdk("explicitly close Device on thread B", device.close());
        merge_results(acquisition, close, "Device cleanup after pull acquisition")
    })?;

    let device = open_device(runtime, serial)?;
    on_thread_b("native-device-drop", move || {
        let mut device = device;
        let mut measurement = sdk("start pull acquisition before Device drop", device.start())?;
        match wait_for_image(&mut measurement) {
            Ok(()) => {
                drop(measurement);
                drop(device);
                Ok(())
            }
            Err(error) => {
                let stop = sdk(
                    "stop pull acquisition after GetImage failure",
                    measurement.stop(),
                );
                let acquisition = merge_results(Err(error), stop, "pull acquisition cleanup");
                let close = sdk("close Device after GetImage failure", device.close());
                merge_results(acquisition, close, "Device cleanup after GetImage failure")
            }
        }
    })
}

fn acquisition_guard_handoff(runtime: &Runtime, serial: &[u8]) -> TestResult {
    let mut device = open_device(runtime, serial)?;
    let measurement = sdk("start Measurement on thread A", device.start())?;
    let worker = on_thread_b("native-measurement-stop", move || {
        let mut measurement = measurement;
        let image = wait_for_image(&mut measurement);
        let stop = sdk("stop moved Measurement on thread B", measurement.stop());
        merge_results(image, stop, "moved Measurement cleanup")
    });
    let close = sdk("close Device after moved Measurement", device.close());
    merge_results(worker, close, "Device cleanup after moved Measurement")?;

    let mut device = open_device(runtime, serial)?;
    let measurement = sdk("start Measurement for moved Drop", device.start())?;
    let worker = on_thread_b("native-measurement-drop", move || {
        drop(measurement);
        Ok(())
    });
    let worker = merge_results(
        worker,
        expect_device_state(&device, DeviceState::Open, "moved Measurement Drop"),
        "Measurement Drop state check",
    );
    let close = sdk("close Device after moved Measurement Drop", device.close());
    merge_results(worker, close, "Device cleanup after Measurement Drop")?;

    let mut device = open_device(runtime, serial)?;
    let (sender, receiver) = mpsc::sync_channel(1);
    let sink: FrameCallbackSink = Arc::new(move |_| match sender.try_send(()) {
        Ok(()) => CallbackDelivery::Delivered,
        Err(mpsc::TrySendError::Full(_)) => CallbackDelivery::Full,
        Err(mpsc::TrySendError::Disconnected(_)) => CallbackDelivery::Disconnected,
    });
    let measurement = sdk(
        "start CallbackMeasurement on thread A",
        device.start_callback(sink),
    )?;
    let worker = on_thread_b("native-callback-stop", move || {
        let received = receiver.recv_timeout(CALLBACK_DEADLINE);
        if received.is_err() {
            let cleanup = sdk(
                "stop CallbackMeasurement after callback timeout",
                measurement.stop(),
            );
            return merge_results(
                Err(String::from(
                    "no native image callback arrived before the deadline",
                )),
                cleanup,
                "CallbackMeasurement timeout cleanup",
            );
        }
        // Receiving from this channel proves that a native frame was copied and reached the
        // sink. Stop may race the sink's return, so it is also the bounded drain assertion.
        sdk(
            "revoke, drain, and stop moved CallbackMeasurement on thread B",
            measurement.stop(),
        )
    });
    let close = sdk("close Device after callback drain", device.close());
    merge_results(worker, close, "Device cleanup after callback drain")?;

    let mut device = open_device(runtime, serial)?;
    let sink: FrameCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
    let measurement = sdk(
        "start CallbackMeasurement for moved Drop",
        device.start_callback(sink),
    )?;
    let worker = on_thread_b("native-callback-drop", move || {
        drop(measurement);
        Ok(())
    });
    let worker = merge_results(
        worker,
        expect_device_state(&device, DeviceState::Open, "moved CallbackMeasurement Drop"),
        "CallbackMeasurement Drop state check",
    );
    let close = sdk(
        "close Device after CallbackMeasurement Drop",
        device.close(),
    );
    merge_results(
        worker,
        close,
        "Device cleanup after CallbackMeasurement Drop",
    )
}

fn exception_callback_handoff(runtime: &Runtime, serial: &[u8]) -> TestResult {
    let mut device = open_device(runtime, serial)?;
    let sink: ExceptionCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
    sdk(
        "register exception callback on thread A",
        device.register_exception_callback(sink),
    )?;
    on_thread_b("native-exception-close", move || {
        sdk(
            "close exception-registered Device on thread B",
            device.close(),
        )
    })
}

fn file_access_started_on_b(
    runtime: &Runtime,
    serial: &[u8],
    device_file: &[u8],
    target: &LocalTarget,
) -> TestResult {
    let device = open_device(runtime, serial)?;
    let local_name = target.sdk_name.clone();
    let device_file = device_file.to_vec();
    on_thread_b("native-file-start", move || {
        let mut device = device;
        let mut transfer = sdk(
            "start read-only FileAccess on thread B",
            device.download_file(&device_file, &local_name),
        )?;
        let completion = complete_transfer(&mut transfer);
        drop(transfer);
        let reuse = completion.and_then(|()| {
            sdk(
                "reuse FileAccess Device on its start thread",
                device.get_parameter(b"ExposureTime"),
            )
            .map(|_| ())
        });
        let close = sdk("close reused FileAccess Device", device.close());
        merge_results(reuse, close, "Device cleanup after FileAccess")
    })
}

fn active_transfer_handoff(
    runtime: &Runtime,
    serial: &[u8],
    device_file: &[u8],
    target: &LocalTarget,
) -> TestResult {
    let mut device = open_device(runtime, serial)?;
    let transfer = sdk(
        "start read-only FileAccess on thread A",
        device.download_file(device_file, &target.sdk_name),
    )?;
    let worker = on_thread_b("native-transfer-handoff", move || {
        let mut transfer = transfer;
        complete_transfer(&mut transfer)
    });
    let reuse = worker.and_then(|()| {
        sdk(
            "reuse Device on thread A after FileAccess guard join",
            device.get_parameter(b"ExposureTime"),
        )
        .map(|_| ())
    });
    let close = sdk("close Device on thread A after guard join", device.close());
    merge_results(reuse, close, "Device cleanup after FileAccess handoff")
}

fn dropped_transfer_resumes(
    runtime: &Runtime,
    serial: &[u8],
    device_file: &[u8],
    target: &LocalTarget,
) -> TestResult {
    let mut device = open_device(runtime, serial)?;
    let worker = on_thread_b("native-transfer-drop", || {
        let transfer = sdk(
            "start FileAccess on thread B before guard Drop",
            device.download_file(device_file, &target.sdk_name),
        )?;
        drop(transfer);
        Ok(())
    });
    let worker = merge_results(
        worker,
        expect_device_state(
            &device,
            DeviceState::Transferring,
            "FileAccess guard Drop on thread B",
        ),
        "FileAccess guard Drop state check",
    );
    let completion = worker.and_then(|()| complete_active_transfer(&mut device));
    let reuse = completion.and_then(|()| {
        sdk(
            "reuse Device on thread A after resumed FileAccess",
            device.get_parameter(b"ExposureTime"),
        )
        .map(|_| ())
    });
    let close = sdk("close Device after resumed FileAccess", device.close());
    merge_results(reuse, close, "Device cleanup after resumed FileAccess")
}

fn immediate_device_cleanup_after_guard_drop(
    runtime: &Runtime,
    serial: &[u8],
    device_file: &[u8],
    close_target: &LocalTarget,
    drop_target: &LocalTarget,
) -> TestResult {
    let mut device = open_device(runtime, serial)?;
    let device_file_for_close = device_file.to_vec();
    let local_name = close_target.sdk_name.clone();
    on_thread_b("native-transfer-close-after-drop", move || {
        let transfer = sdk(
            "start FileAccess before immediate guard Drop and Device close",
            device.download_file(&device_file_for_close, &local_name),
        )?;
        drop(transfer);
        sdk(
            "close Device immediately after FileAccess guard Drop",
            device.close(),
        )
    })?;

    let mut device = open_device(runtime, serial)?;
    let device_file_for_drop = device_file.to_vec();
    let local_name = drop_target.sdk_name.clone();
    on_thread_b("native-transfer-device-drop", move || {
        let transfer = sdk(
            "start FileAccess before immediate guard and Device Drop",
            device.download_file(&device_file_for_drop, &local_name),
        )?;
        drop(transfer);
        drop(device);
        Ok(())
    })
}

fn complete_transfer(transfer: &mut FileTransfer<'_>) -> TestResult {
    match transfer.wait_timeout(FILE_POLL_INTERVAL, FILE_DEADLINE) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(String::from("FileAccess exceeded its total deadline")),
        Err(error) => Err(format!("FileAccess progress failed: {error}")),
    }
}

fn complete_active_transfer(device: &mut Device<'_>) -> TestResult {
    let mut transfer = device.active_file_transfer().ok_or_else(|| {
        String::from("FileAccess guard Drop did not leave an active transfer to resume")
    })?;
    complete_transfer(&mut transfer)
}

fn wait_for_image(measurement: &mut Measurement<'_>) -> TestResult {
    let deadline = Instant::now() + GET_IMAGE_DEADLINE;
    loop {
        match measurement.get_image(GET_IMAGE_SLICE_MS) {
            Ok(_) => return Ok(()),
            Err(Error::Sdk { status, .. }) if status as u32 == NO_DATA_STATUS => {
                if Instant::now() >= deadline {
                    return Err(String::from(
                        "MV3D_LP_GetImage returned no data until the total deadline",
                    ));
                }
            }
            Err(error) => return Err(format!("MV3D_LP_GetImage failed: {error}")),
        }
    }
}

fn expect_device_state(
    device: &Device<'_>,
    expected: DeviceState,
    context: &'static str,
) -> TestResult {
    if device.state() == expected {
        Ok(())
    } else {
        Err(format!(
            "{context} left the Device in state {:?}, expected {expected:?}",
            device.state()
        ))
    }
}

fn selected_device_metadata(runtime: &Runtime, serial: &[u8]) -> TestResult<(String, String)> {
    let devices = sdk(
        "enumerate devices after the version check",
        runtime.devices(),
    )?;
    let Some(device) = devices.iter().find(|device| device.serial_number == serial) else {
        return Err(format!(
            "the configured serial did not match any of the {} discovered devices",
            devices.len()
        ));
    };
    Ok((
        printable_label(&device.model_name),
        redacted_identifier(serial),
    ))
}

fn open_device<'runtime>(
    runtime: &'runtime Runtime,
    serial: &[u8],
) -> TestResult<Device<'runtime>> {
    sdk(
        "open the configured device by its redacted serial",
        runtime.open_by_serial(serial),
    )
}

fn on_thread_b<T: Send>(
    name: &'static str,
    work: impl FnOnce() -> TestResult<T> + Send,
) -> TestResult<T> {
    thread::scope(|scope| {
        let worker = thread::Builder::new()
            .name(String::from(name))
            .spawn_scoped(scope, move || {
                println!(
                    "native acceptance: thread_b={:?} scenario={name}",
                    thread::current().id()
                );
                work()
            })
            .map_err(|error| format!("could not spawn thread B: {error}"))?;
        worker
            .join()
            .map_err(|_| String::from("thread B panicked during native acceptance"))?
    })
}

fn scenario(name: &'static str, run: impl FnOnce() -> TestResult) -> TestResult {
    println!("native acceptance: RUN {name}");
    run()?;
    println!("native acceptance: PASS {name}");
    Ok(())
}

fn sdk<T, E: Display>(context: &'static str, result: Result<T, E>) -> TestResult<T> {
    result.map_err(|error| format!("{context}: {error}"))
}

fn merge_results(
    primary: TestResult,
    cleanup: TestResult,
    cleanup_context: &'static str,
) -> TestResult {
    match (primary, cleanup) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup)) => Err(format!("{cleanup_context}: {cleanup}")),
        (Err(error), Err(cleanup)) => {
            Err(format!("{error}; {cleanup_context} also failed: {cleanup}"))
        }
    }
}

fn required_ascii_env(name: &'static str, maximum: Option<usize>) -> TestResult<Vec<u8>> {
    let value =
        env::var(name).map_err(|_| format!("{name} must be set to a non-empty ASCII value"))?;
    if value.is_empty() || !value.is_ascii() {
        return Err(format!("{name} must be set to a non-empty ASCII value"));
    }
    if maximum.is_some_and(|maximum| value.len() > maximum) {
        return Err(format!("{name} exceeds the LPSDK field limit"));
    }
    Ok(value.into_bytes())
}

fn printable_label(bytes: &[u8]) -> String {
    let label: String = bytes
        .iter()
        .take(64)
        .map(|byte| {
            if byte.is_ascii_graphic() || *byte == b' ' {
                char::from(*byte)
            } else {
                '?'
            }
        })
        .collect();
    if label.is_empty() {
        String::from("<empty>")
    } else {
        label
    }
}

fn redacted_identifier(serial: &[u8]) -> String {
    const VISIBLE_SUFFIX: usize = 4;
    if serial.len() <= VISIBLE_SUFFIX {
        return "*".repeat(serial.len());
    }
    let suffix = printable_label(&serial[serial.len() - VISIBLE_SUFFIX..]);
    format!("***{suffix}")
}

struct LocalTarget {
    path: PathBuf,
    sdk_name: Vec<u8>,
}

struct ScratchDirectory {
    run_directory: Option<PathBuf>,
    targets: Vec<PathBuf>,
}

impl ScratchDirectory {
    fn from_env() -> TestResult<Self> {
        let value = env::var(SCRATCH_ENV).map_err(|_| {
            format!("{SCRATCH_ENV} must name a pre-existing empty absolute ASCII directory")
        })?;
        if value.is_empty() || !value.is_ascii() {
            return Err(format!(
                "{SCRATCH_ENV} must name a pre-existing empty absolute ASCII directory"
            ));
        }
        let root = PathBuf::from(value);
        if !root.is_absolute()
            || root
                .components()
                .any(|component| matches!(component, Component::CurDir | Component::ParentDir))
        {
            return Err(format!(
                "{SCRATCH_ENV} must be absolute and contain no dot components"
            ));
        }
        let metadata = fs::symlink_metadata(&root)
            .map_err(|error| format!("could not inspect {SCRATCH_ENV}: {error}"))?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(format!("{SCRATCH_ENV} must identify a real directory"));
        }
        let mut entries = fs::read_dir(&root)
            .map_err(|error| format!("could not read {SCRATCH_ENV}: {error}"))?;
        if entries
            .next()
            .transpose()
            .map_err(|error| format!("could not verify that {SCRATCH_ENV} is empty: {error}"))?
            .is_some()
        {
            return Err(format!("{SCRATCH_ENV} must be empty before the test"));
        }

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| String::from("the system clock is earlier than the Unix epoch"))?
            .as_nanos();
        let run_directory = root.join(format!("mv3d-lp-native-{}-{nonce}", std::process::id()));
        fs::create_dir(&run_directory).map_err(|error| {
            format!("could not create the unique scratch subdirectory: {error}")
        })?;

        Ok(Self {
            run_directory: Some(run_directory),
            targets: Vec::new(),
        })
    }

    fn target(&mut self, file_name: &str) -> TestResult<LocalTarget> {
        let directory = self
            .run_directory
            .as_ref()
            .ok_or_else(|| String::from("the scratch directory was already cleaned"))?;
        let path = directory.join(file_name);
        if path.exists() {
            return Err(String::from(
                "a supposedly unique scratch target already exists",
            ));
        }
        let sdk_name = path
            .to_str()
            .filter(|value| value.is_ascii())
            .ok_or_else(|| String::from("the local scratch target is not an ASCII path"))?
            .as_bytes()
            .to_vec();
        self.targets.push(path.clone());
        Ok(LocalTarget { path, sdk_name })
    }

    fn cleanup(&mut self) -> TestResult {
        let mut failures = Vec::new();
        for target in &self.targets {
            match fs::symlink_metadata(target) {
                Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => {
                    if let Err(error) = fs::remove_file(target) {
                        failures.push(format!("could not remove a test output file: {error}"));
                    }
                }
                Ok(_) => failures.push(String::from(
                    "a test output path was not a regular file and was left untouched",
                )),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => {
                    failures.push(format!("could not inspect a test output file: {error}"))
                }
            }
        }

        if failures.is_empty() {
            if let Some(directory) = self.run_directory.take() {
                if let Err(error) = fs::remove_dir(&directory) {
                    self.run_directory = Some(directory);
                    failures.push(format!(
                        "could not remove the scratch subdirectory: {error}"
                    ));
                }
            }
        }

        if failures.is_empty() {
            self.targets.clear();
            Ok(())
        } else {
            Err(failures.join("; "))
        }
    }
}

impl Drop for ScratchDirectory {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

fn require_regular_file(path: &Path, description: &'static str) -> TestResult {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => Ok(()),
        Ok(_) => Err(format!("{description} is not a regular file")),
        Err(error) => Err(format!("{description} was not created: {error}")),
    }
}

struct ProcessDeadline {
    finished: Arc<AtomicBool>,
    watchdog: Option<thread::JoinHandle<()>>,
}

impl ProcessDeadline {
    fn start(timeout: Duration) -> Self {
        let finished = Arc::new(AtomicBool::new(false));
        let watchdog_finished = Arc::clone(&finished);
        let watchdog = thread::spawn(move || {
            let deadline = Instant::now() + timeout;
            loop {
                if watchdog_finished.load(Ordering::Acquire) {
                    return;
                }
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    eprintln!(
                        "LPSDK native acceptance exceeded its process deadline; aborting the isolated test process"
                    );
                    std::process::abort();
                }
                thread::park_timeout(remaining);
            }
        });
        Self {
            finished,
            watchdog: Some(watchdog),
        }
    }
}

impl Drop for ProcessDeadline {
    fn drop(&mut self) {
        self.finished.store(true, Ordering::Release);
        if let Some(watchdog) = self.watchdog.take() {
            watchdog.thread().unpark();
            let _ = watchdog.join();
        }
    }
}
