#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::ffi::CString;
use std::marker::PhantomData;
use std::net::Ipv4Addr;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::device::{DeviceRecord, IpConfigRaw, IpConfiguration};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{Driver, DriverError, DriverResult, Handle};
use crate::error::{ContractViolation, Error, InvalidInput, Operation};
use crate::frame::{FrameRecord, ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::opened_device::Device;

const AUDITED_VERSION_TEXT: &[u8] = b"1.3.3.3";
const MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE_TEXT: &[u8] = b"1.3.4.0";
const AUDITED_VERSION: SdkVersion = SdkVersion::new(1, 3, 3, 3);
const MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE: SdkVersion = SdkVersion::new(1, 3, 4, 0);
const MAX_DEVICE_COUNT: usize = 256;
const DISCOVERY_ATTEMPTS: usize = 3;
const STATUS_BUFFER_FULL: i32 = 0x8006_0002_u32 as i32;
const STATUS_INSUFFICIENT_BUFFER: i32 = 0x8006_0009_u32 as i32;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct SdkVersion([u32; 4]);

impl SdkVersion {
    const fn new(major: u32, minor: u32, patch: u32, build: u32) -> Self {
        Self([major, minor, patch, build])
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum VersionPolicy {
    Compatible,
    Strict,
}

impl VersionPolicy {
    fn accepts(self, version: SdkVersion) -> bool {
        match self {
            Self::Compatible => {
                (AUDITED_VERSION..MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE).contains(&version)
            }
            Self::Strict => version == AUDITED_VERSION,
        }
    }

    const fn maximum_exclusive(self) -> Option<&'static [u8]> {
        match self {
            Self::Compatible => Some(MAXIMUM_COMPATIBLE_VERSION_EXCLUSIVE_TEXT),
            Self::Strict => None,
        }
    }
}

/// Process-wide native LPSDK session state shared by every `Runtime` instance.
///
/// This is deliberately separate from a device's `DeviceState`. `Degraded` blocks lifecycle
/// expansion and finalization, but existing Rust values remain usable until their owner is gone.
/// `live_handles` counts handles still tracked by Rust; a degraded native session may additionally
/// retain orphaned handles that cannot be counted safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessSdkState {
    Fresh,
    Active { live_handles: usize },
    Degraded { live_handles: usize },
}

pub(crate) struct Gate {
    // This state belongs to the process-wide native LPSDK session and survives individual
    // Runtime values. Per-device lifecycle is tracked separately by DeviceState.
    state: Mutex<ProcessSdkState>,
}

impl Gate {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(ProcessSdkState::Fresh),
        }
    }

    fn lock(&self) -> MutexGuard<'_, ProcessSdkState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                // A panic while this gate is held may leave a native lifecycle side effect ahead
                // of the Rust ledger. Preserve the tracked count but forbid further expansion.
                let mut state = poisoned.into_inner();
                let live_handles = match *state {
                    ProcessSdkState::Fresh => 0,
                    ProcessSdkState::Active { live_handles }
                    | ProcessSdkState::Degraded { live_handles } => live_handles,
                };
                *state = ProcessSdkState::Degraded { live_handles };
                self.state.clear_poison();
                state
            }
        }
    }
}

pub(crate) struct RuntimeInner {
    driver: Box<dyn Driver>,
    gate: Arc<Gate>,
    image_processing: Mutex<()>,
}

impl RuntimeInner {
    // RuntimeInner is created only after Initialize succeeds, and every value that can call these
    // methods borrows it. Rust therefore prevents Finalize while an existing call-capable value
    // is alive; ordinary and image calls do not need to consult the process lifecycle gate.
    pub(crate) fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        call(self.driver.as_ref()).map_err(|error| map_driver_error(operation, error))
    }

    fn image_call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let _image_processing = self
            .image_processing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        call(self.driver.as_ref()).map_err(|error| map_driver_error(operation, error))
    }

    pub(crate) fn open_handle(
        &self,
        operation: Operation,
        open: impl FnOnce(&dyn Driver, &mut Option<Handle>) -> DriverResult<()>,
    ) -> Result<Handle, Error> {
        let mut state = self.gate.lock();
        let live_handles = match *state {
            ProcessSdkState::Active { live_handles } => live_handles,
            ProcessSdkState::Degraded { .. } => return Err(Error::RuntimeDegraded),
            ProcessSdkState::Fresh => return Err(Error::RuntimeTerminal),
        };

        let mut handle = None;
        match open(self.driver.as_ref(), &mut handle) {
            Ok(()) => {
                let handle = handle.ok_or(Error::ContractViolation {
                    operation,
                    kind: ContractViolation::NullHandleOnSuccess,
                })?;
                let Some(live_handles) = live_handles.checked_add(1) else {
                    *state = ProcessSdkState::Degraded { live_handles };
                    return Err(Error::ContractViolation {
                        operation,
                        kind: ContractViolation::HandleCountOverflow,
                    });
                };
                *state = ProcessSdkState::Active { live_handles };
                Ok(handle)
            }
            Err(error) if handle.is_some() => {
                *state = ProcessSdkState::Degraded { live_handles };
                Err(Error::OpenFailedWithHandle {
                    operation,
                    source: Box::new(map_driver_error(operation, error)),
                })
            }
            Err(error) => Err(map_driver_error(operation, error)),
        }
    }

    pub(crate) fn cleanup_close_handle(&self, handle: Handle) -> Result<(), Error> {
        const OPERATION: Operation = Operation::CloseDevice;

        let mut state = self.gate.lock();
        let (live_handles, was_degraded) = match *state {
            ProcessSdkState::Active { live_handles } => (live_handles, false),
            ProcessSdkState::Degraded { live_handles } => (live_handles, true),
            ProcessSdkState::Fresh => return Err(Error::RuntimeTerminal),
        };

        let result = self
            .driver
            .close(handle)
            .map_err(|error| map_driver_error(OPERATION, error));
        let (live_handles, ledger_uncertain) = match live_handles.checked_sub(1) {
            Some(live_handles) => (live_handles, false),
            None => (live_handles, true),
        };
        *state = if was_degraded || ledger_uncertain || result.is_err() {
            ProcessSdkState::Degraded { live_handles }
        } else {
            ProcessSdkState::Active { live_handles }
        };
        result
    }

    pub(crate) fn parameter_key(operation: Operation, bytes: &[u8]) -> Result<CString, Error> {
        validated_c_string(operation, bytes, 255, true)
    }
}

pub struct Runtime {
    inner: RuntimeInner,
    version: Vec<u8>,
    finished: bool,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Runtime {
    pub fn initialize() -> Result<Self, Error> {
        Self::initialize_native(VersionPolicy::Compatible)
    }

    pub fn initialize_strict() -> Result<Self, Error> {
        Self::initialize_native(VersionPolicy::Strict)
    }

    fn initialize_native(policy: VersionPolicy) -> Result<Self, Error> {
        #[cfg(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        ))]
        {
            static GATE: OnceLock<Arc<Gate>> = OnceLock::new();
            let gate = Arc::clone(GATE.get_or_init(|| Arc::new(Gate::new())));
            Self::initialize_with_policy(Box::new(crate::ffi::NativeDriver), gate, policy)
        }

        #[cfg(not(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )))]
        {
            let _ = (OnceLock::<Arc<Gate>>::new(), policy);
            Err(Error::UnsupportedPlatform)
        }
    }

    #[cfg(test)]
    pub(crate) fn initialize_with(driver: Box<dyn Driver>, gate: Arc<Gate>) -> Result<Self, Error> {
        Self::initialize_with_policy(driver, gate, VersionPolicy::Compatible)
    }

    #[cfg(test)]
    pub(crate) fn initialize_with_strict(
        driver: Box<dyn Driver>,
        gate: Arc<Gate>,
    ) -> Result<Self, Error> {
        Self::initialize_with_policy(driver, gate, VersionPolicy::Strict)
    }

    fn initialize_with_policy(
        driver: Box<dyn Driver>,
        gate: Arc<Gate>,
        policy: VersionPolicy,
    ) -> Result<Self, Error> {
        let mut state = gate.lock();
        match *state {
            ProcessSdkState::Fresh => {}
            ProcessSdkState::Active { .. } => return Err(Error::RuntimeAlreadyActive),
            ProcessSdkState::Degraded { .. } => return Err(Error::RuntimeDegraded),
        }

        let version = match driver.version() {
            Ok(version) => version,
            Err(error) => return Err(map_driver_error(Operation::GetVersion, error)),
        };
        let parsed_version = parse_sdk_version(&version);
        if !parsed_version.is_some_and(|version| policy.accepts(version)) {
            return Err(Error::IncompatibleSdkVersion {
                minimum: AUDITED_VERSION_TEXT,
                maximum_exclusive: policy.maximum_exclusive(),
                actual: version,
            });
        }

        if let Err(error) = driver.initialize() {
            let initialize = map_driver_error(Operation::Initialize, error);
            // Initialize may have partially established process-wide native state. Preserve its
            // primary error, but only leave the session Fresh when compensating cleanup succeeds.
            if driver.finalize().is_err() {
                *state = ProcessSdkState::Degraded { live_handles: 0 };
            }
            return Err(initialize);
        }

        *state = ProcessSdkState::Active { live_handles: 0 };
        drop(state);
        Ok(Self {
            inner: RuntimeInner {
                driver,
                gate,
                image_processing: Mutex::new(()),
            },
            version,
            finished: false,
            _not_send_or_sync: PhantomData,
        })
    }

    pub fn version_bytes(&self) -> &[u8] {
        &self.version
    }

    #[cfg(test)]
    pub(crate) fn set_live_handles_for_test(&self, live_handles: usize) {
        let mut state = self.inner.gate.lock();
        match &mut *state {
            ProcessSdkState::Active {
                live_handles: current,
            }
            | ProcessSdkState::Degraded {
                live_handles: current,
            } => *current = live_handles,
            ProcessSdkState::Fresh => panic!("an initialized Runtime cannot have a Fresh gate"),
        }
    }

    pub fn device_count_hint(&self) -> Result<u32, Error> {
        self.call(Operation::GetDeviceNumber, |driver| driver.device_number())
    }

    pub fn devices(&self) -> Result<Vec<DeviceRecord>, Error> {
        let hint = self.device_count_hint()?;
        validate_device_count(Operation::GetDeviceNumber, hint)?;
        let mut capacity = usize::try_from(hint).unwrap_or(usize::MAX).max(1);

        for attempt in 1..=DISCOVERY_ATTEMPTS {
            let list = self.call(Operation::GetDeviceList, |driver| {
                driver.device_list(capacity)
            });

            let list = match list {
                Ok(list) => list,
                Err(Error::Sdk { status, .. })
                    if status == STATUS_BUFFER_FULL || status == STATUS_INSUFFICIENT_BUFFER =>
                {
                    if attempt == DISCOVERY_ATTEMPTS {
                        break;
                    }
                    let refreshed = self.device_count_hint()?;
                    validate_device_count(Operation::GetDeviceNumber, refreshed)?;
                    capacity = grow_capacity(capacity, refreshed)?;
                    continue;
                }
                Err(error) => return Err(error),
            };

            validate_device_count(Operation::GetDeviceList, list.reported)?;
            let reported = usize::try_from(list.reported).unwrap_or(usize::MAX);
            if reported > capacity {
                if attempt == DISCOVERY_ATTEMPTS {
                    break;
                }
                capacity = grow_capacity(capacity, list.reported)?;
                continue;
            }
            if list.records.len() != reported {
                return Err(Error::ContractViolation {
                    operation: Operation::GetDeviceList,
                    kind: ContractViolation::DeviceListCountMismatch {
                        reported: list.reported,
                        returned: list.records.len(),
                    },
                });
            }

            let mut records = Vec::new();
            records
                .try_reserve_exact(reported)
                .map_err(|_| Error::AllocationFailed {
                    operation: Operation::GetDeviceList,
                    requested: reported,
                })?;
            records.extend(list.records.into_iter().map(DeviceRecord::from));
            return Ok(records);
        }

        Err(Error::DiscoveryChanged {
            attempts: DISCOVERY_ATTEMPTS,
        })
    }

    pub fn set_ip_config(
        &self,
        serial_number: &[u8],
        configuration: &IpConfiguration,
    ) -> Result<(), Error> {
        let serial = validated_c_string(Operation::SetIpConfig, serial_number, 16, false)?;
        let raw = IpConfigRaw::from(configuration);
        self.call(Operation::SetIpConfig, |driver| {
            driver.set_ip_config(&serial, &raw)
        })
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Device<'_>, Error> {
        let address = CString::new(address.to_string()).expect("an IPv4 address contains no NUL");
        let handle = self.open(Operation::OpenDeviceByIp, |driver, output| {
            driver.open_by_ip(&address, output)
        })?;
        Ok(Device::new(&self.inner, handle))
    }

    pub fn open_by_serial(&self, serial_number: &[u8]) -> Result<Device<'_>, Error> {
        let serial = validated_c_string(Operation::OpenDeviceBySn, serial_number, 16, false)?;
        let handle = self.open(Operation::OpenDeviceBySn, |driver, output| {
            driver.open_by_serial(&serial, output)
        })?;
        Ok(Device::new(&self.inner, handle))
    }

    pub fn map_depth_to_point_cloud(&self, input: ImageInput<'_>) -> Result<FrameRecord, Error> {
        self.inner
            .image_call(Operation::MapDepthToPointCloud, |driver| {
                driver.map_depth_to_point_cloud(input)
            })
    }

    pub fn map_depth_to_point_cloud_round(
        &self,
        inputs: &[ImageInput<'_>],
    ) -> Result<FrameRecord, Error> {
        self.inner
            .image_call(Operation::MapDepthToPointCloudRound, |driver| {
                driver.map_depth_to_point_cloud_round(inputs)
            })
    }

    pub fn convert_image(
        &self,
        input: ImageInput<'_>,
        target: ImageTypeRecord,
    ) -> Result<FrameRecord, Error> {
        self.inner.image_call(Operation::ImageConvert, |driver| {
            driver.convert_image(input, target)
        })
    }

    pub fn mosaic_depth(&self, inputs: &[ImageInput<'_>]) -> Result<FrameRecord, Error> {
        self.inner
            .image_call(Operation::DepthMosaic, |driver| driver.mosaic_depth(inputs))
    }

    pub fn save_image(
        &self,
        input: ImageInput<'_>,
        format: ImageFileFormatRecord,
        file_name: &[u8],
    ) -> Result<(), Error> {
        let file_name =
            validated_c_string(Operation::SaveImage, file_name, u32::MAX as usize, false)?;
        self.inner.image_call(Operation::SaveImage, |driver| {
            driver.save_image(input, format, &file_name)
        })
    }

    #[cfg(feature = "display-windows")]
    pub fn display_image(
        &self,
        input: ImageInput<'_>,
        window: NonZeroIsize,
        range: DisplayRangeRecord,
    ) -> Result<(), Error> {
        self.inner.image_call(Operation::DisplayImage, |driver| {
            driver.display_image(input, window, range)
        })
    }

    pub fn shutdown(mut self) -> Result<(), Error> {
        self.finish()
    }

    fn open(
        &self,
        operation: Operation,
        open: impl FnOnce(&dyn Driver, &mut Option<Handle>) -> DriverResult<()>,
    ) -> Result<Handle, Error> {
        self.inner.open_handle(operation, open)
    }

    fn call<T>(
        &self,
        operation: Operation,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        self.inner.call(operation, call)
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.finished {
            return Err(Error::RuntimeTerminal);
        }
        self.finished = true;
        let mut state = self.inner.gate.lock();
        match *state {
            ProcessSdkState::Fresh => return Err(Error::RuntimeTerminal),
            ProcessSdkState::Active { live_handles: 0 } => {}
            ProcessSdkState::Active { live_handles } => {
                *state = ProcessSdkState::Degraded { live_handles };
                return Err(Error::UnclosedDevices {
                    live_handles,
                    teardown_uncertain: false,
                });
            }
            ProcessSdkState::Degraded { live_handles } => {
                return Err(Error::UnclosedDevices {
                    live_handles,
                    teardown_uncertain: true,
                });
            }
        }
        match self.inner.driver.finalize() {
            Ok(()) => {
                *state = ProcessSdkState::Fresh;
                Ok(())
            }
            Err(error) => {
                *state = ProcessSdkState::Degraded { live_handles: 0 };
                Err(map_driver_error(Operation::Finalize, error))
            }
        }
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

fn parse_sdk_version(bytes: &[u8]) -> Option<SdkVersion> {
    let mut components = std::str::from_utf8(bytes).ok()?.split('.');
    let major = components.next()?.parse().ok()?;
    let minor = components.next()?.parse().ok()?;
    let patch = components.next()?.parse().ok()?;
    let build = components.next()?.parse().ok()?;
    if components.next().is_some() {
        return None;
    }
    Some(SdkVersion::new(major, minor, patch, build))
}

fn map_driver_error(operation: Operation, error: DriverError) -> Error {
    match error {
        DriverError::Status(status) => Error::Sdk { operation, status },
        DriverError::InvalidInput(kind) => Error::InvalidInput { operation, kind },
        DriverError::Contract(kind) => Error::ContractViolation { operation, kind },
        DriverError::Allocation { requested } => Error::AllocationFailed {
            operation,
            requested,
        },
    }
}

fn validate_device_count(operation: Operation, count: u32) -> Result<(), Error> {
    if usize::try_from(count).unwrap_or(usize::MAX) > MAX_DEVICE_COUNT {
        Err(Error::ContractViolation {
            operation,
            kind: ContractViolation::DeviceCountExceedsLimit {
                reported: count,
                limit: MAX_DEVICE_COUNT,
            },
        })
    } else {
        Ok(())
    }
}

fn grow_capacity(current: usize, reported: u32) -> Result<usize, Error> {
    validate_device_count(Operation::GetDeviceList, reported)?;
    let reported = usize::try_from(reported).unwrap_or(usize::MAX);
    let doubled = current.saturating_mul(2).min(MAX_DEVICE_COUNT);
    let next = reported.max(doubled).max(current.saturating_add(1));
    if next > MAX_DEVICE_COUNT {
        return Err(Error::ContractViolation {
            operation: Operation::GetDeviceList,
            kind: ContractViolation::DeviceCountExceedsLimit {
                reported: u32::try_from(next).unwrap_or(u32::MAX),
                limit: MAX_DEVICE_COUNT,
            },
        });
    }
    Ok(next)
}

fn validated_c_string(
    operation: Operation,
    bytes: &[u8],
    maximum: usize,
    require_ascii: bool,
) -> Result<CString, Error> {
    if bytes.is_empty() {
        return Err(Error::InvalidInput {
            operation,
            kind: InvalidInput::Empty,
        });
    }
    if bytes.len() > maximum {
        return Err(Error::InvalidInput {
            operation,
            kind: InvalidInput::TooLong {
                actual: bytes.len(),
                maximum,
            },
        });
    }
    if require_ascii && !bytes.is_ascii() {
        return Err(Error::InvalidInput {
            operation,
            kind: InvalidInput::NonAscii,
        });
    }
    CString::new(bytes).map_err(|_| Error::InvalidInput {
        operation,
        kind: InvalidInput::InteriorNul,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Gate, ProcessSdkState};

    #[test]
    fn poisoned_lifecycle_gate_fails_closed() {
        let gate = Arc::new(Gate::new());
        let poisoning_gate = Arc::clone(&gate);

        assert!(
            std::thread::spawn(move || {
                let _state = poisoning_gate.state.lock().unwrap();
                panic!("poison the lifecycle gate");
            })
            .join()
            .is_err()
        );

        assert_eq!(*gate.lock(), ProcessSdkState::Degraded { live_handles: 0 });
        assert!(!gate.state.is_poisoned());
    }
}
