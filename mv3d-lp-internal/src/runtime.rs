#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::cell::Cell;
use std::ffi::CString;
use std::marker::PhantomData;
use std::net::Ipv4Addr;
use std::rc::Rc;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::camera::Camera;
use crate::device::{DeviceRecord, IpConfigRaw, IpConfiguration};
use crate::driver::{Driver, DriverError, DriverResult, Handle};
use crate::error::{ContractViolation, Error, InvalidInput};

const EXPECTED_VERSION: &[u8] = b"1.3.3.3";
const MAX_DEVICE_COUNT: usize = 256;
const DISCOVERY_ATTEMPTS: usize = 3;
const STATUS_BUFFER_FULL: i32 = 0x8006_0002_u32 as i32;
const STATUS_INSUFFICIENT_BUFFER: i32 = 0x8006_0009_u32 as i32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProcessState {
    Fresh,
    Active,
    Terminal,
}

pub(crate) struct Gate {
    state: Mutex<ProcessState>,
}

impl Gate {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(ProcessState::Fresh),
        }
    }

    fn lock(&self) -> MutexGuard<'_, ProcessState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

pub struct Runtime {
    driver: Box<dyn Driver>,
    gate: Arc<Gate>,
    version: Vec<u8>,
    finished: bool,
    live_handles: Cell<usize>,
    teardown_uncertain: Cell<bool>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl Runtime {
    pub fn initialize() -> Result<Self, Error> {
        #[cfg(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        ))]
        {
            static GATE: OnceLock<Arc<Gate>> = OnceLock::new();
            let gate = Arc::clone(GATE.get_or_init(|| Arc::new(Gate::new())));
            Self::initialize_with(Box::new(crate::ffi::NativeDriver), gate)
        }

        #[cfg(not(all(
            feature = "native",
            target_os = "windows",
            target_arch = "x86_64",
            target_env = "msvc"
        )))]
        {
            let _ = OnceLock::<Arc<Gate>>::new();
            Err(Error::UnsupportedPlatform)
        }
    }

    pub(crate) fn initialize_with(driver: Box<dyn Driver>, gate: Arc<Gate>) -> Result<Self, Error> {
        let mut state = gate.lock();
        match *state {
            ProcessState::Fresh => {}
            ProcessState::Active => return Err(Error::RuntimeAlreadyActive),
            ProcessState::Terminal => return Err(Error::RuntimeTerminal),
        }

        let version = match driver.version() {
            Ok(version) => version,
            Err(error) => {
                *state = ProcessState::Terminal;
                return Err(map_driver_error("MV3D_LP_GetVersion", error));
            }
        };
        if version.as_slice() != EXPECTED_VERSION {
            *state = ProcessState::Terminal;
            return Err(Error::IncompatibleSdkVersion {
                expected: EXPECTED_VERSION,
                actual: version,
            });
        }

        if let Err(error) = driver.initialize() {
            *state = ProcessState::Terminal;
            return Err(map_driver_error("MV3D_LP_Initialize", error));
        }

        *state = ProcessState::Active;
        drop(state);
        Ok(Self {
            driver,
            gate,
            version,
            finished: false,
            live_handles: Cell::new(0),
            teardown_uncertain: Cell::new(false),
            _not_send_or_sync: PhantomData,
        })
    }

    pub fn version_bytes(&self) -> &[u8] {
        &self.version
    }

    pub fn device_count_hint(&self) -> Result<u32, Error> {
        self.call("MV3D_LP_GetDeviceNumber", |driver| driver.device_number())
    }

    pub fn devices(&self) -> Result<Vec<DeviceRecord>, Error> {
        let hint = self.device_count_hint()?;
        validate_device_count("MV3D_LP_GetDeviceNumber", hint)?;
        let mut capacity = usize::try_from(hint).unwrap_or(usize::MAX).max(1);

        for attempt in 1..=DISCOVERY_ATTEMPTS {
            let list = self.call("MV3D_LP_GetDeviceList", |driver| {
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
                    validate_device_count("MV3D_LP_GetDeviceNumber", refreshed)?;
                    capacity = grow_capacity(capacity, refreshed)?;
                    continue;
                }
                Err(error) => return Err(error),
            };

            validate_device_count("MV3D_LP_GetDeviceList", list.reported)?;
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
                    operation: "MV3D_LP_GetDeviceList",
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
        let serial = validated_c_string("MV3D_LP_SetIpConfig", serial_number, 16, false)?;
        let raw = IpConfigRaw::from(configuration);
        self.call("MV3D_LP_SetIpConfig", |driver| {
            driver.set_ip_config(&serial, &raw)
        })
    }

    pub fn open_by_ip(&self, address: Ipv4Addr) -> Result<Camera<'_>, Error> {
        let address = CString::new(address.to_string()).expect("an IPv4 address contains no NUL");
        let handle = self.open("MV3D_LP_OpenDeviceByIP", |driver, output| {
            driver.open_by_ip(&address, output)
        })?;
        Ok(Camera::new(self, handle))
    }

    pub fn open_by_serial(&self, serial_number: &[u8]) -> Result<Camera<'_>, Error> {
        let serial = validated_c_string("MV3D_LP_OpenDeviceBySN", serial_number, 16, false)?;
        let handle = self.open("MV3D_LP_OpenDeviceBySN", |driver, output| {
            driver.open_by_serial(&serial, output)
        })?;
        Ok(Camera::new(self, handle))
    }

    pub fn shutdown(mut self) -> Result<(), Error> {
        self.finish()
    }

    fn open(
        &self,
        operation: &'static str,
        open: impl FnOnce(&dyn Driver, &mut Option<Handle>) -> DriverResult<()>,
    ) -> Result<Handle, Error> {
        let handle = self.call(operation, |driver| {
            let mut handle = None;
            match open(driver, &mut handle) {
                Ok(()) => handle.ok_or(DriverError::Contract(
                    ContractViolation::NullHandleOnSuccess,
                )),
                Err(error) => {
                    if handle.is_some() {
                        self.teardown_uncertain.set(true);
                        Err(DriverError::OrphanedHandle(Box::new(error)))
                    } else {
                        Err(error)
                    }
                }
            }
        })?;
        self.register_handle(operation)?;
        Ok(handle)
    }

    pub(crate) fn call<T>(
        &self,
        operation: &'static str,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let state = self.gate.lock();
        if *state != ProcessState::Active || self.teardown_uncertain.get() {
            return Err(Error::RuntimeTerminal);
        }
        let result = call(self.driver.as_ref()).map_err(|error| map_driver_error(operation, error));
        drop(state);
        result
    }

    pub(crate) fn cleanup_call<T>(
        &self,
        operation: &'static str,
        call: impl FnOnce(&dyn Driver) -> DriverResult<T>,
    ) -> Result<T, Error> {
        let state = self.gate.lock();
        if *state != ProcessState::Active {
            return Err(Error::RuntimeTerminal);
        }
        let result = call(self.driver.as_ref()).map_err(|error| map_driver_error(operation, error));
        drop(state);
        result
    }

    pub(crate) fn parameter_key(operation: &'static str, bytes: &[u8]) -> Result<CString, Error> {
        validated_c_string(operation, bytes, 255, true)
    }

    pub(crate) fn record_close_result(&self, close_succeeded: bool) {
        match self.live_handles.get().checked_sub(1) {
            Some(remaining) => self.live_handles.set(remaining),
            None => self.teardown_uncertain.set(true),
        }
        if !close_succeeded {
            self.teardown_uncertain.set(true);
        }
    }

    fn register_handle(&self, operation: &'static str) -> Result<(), Error> {
        match self.live_handles.get().checked_add(1) {
            Some(count) => {
                self.live_handles.set(count);
                Ok(())
            }
            None => {
                self.teardown_uncertain.set(true);
                Err(Error::ContractViolation {
                    operation,
                    kind: ContractViolation::HandleCountOverflow,
                })
            }
        }
    }

    fn finish(&mut self) -> Result<(), Error> {
        if self.finished {
            return Err(Error::RuntimeTerminal);
        }
        self.finished = true;
        let mut state = self.gate.lock();
        if *state != ProcessState::Active {
            *state = ProcessState::Terminal;
            return Err(Error::RuntimeTerminal);
        }
        let live_handles = self.live_handles.get();
        let teardown_uncertain = self.teardown_uncertain.get();
        if live_handles != 0 || teardown_uncertain {
            *state = ProcessState::Terminal;
            return Err(Error::UnclosedDevices {
                live_handles,
                teardown_uncertain,
            });
        }
        let result = self
            .driver
            .finalize()
            .map_err(|error| map_driver_error("MV3D_LP_Finalize", error));
        *state = ProcessState::Terminal;
        result
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        if !self.finished {
            let _ = self.finish();
        }
    }
}

fn map_driver_error(operation: &'static str, error: DriverError) -> Error {
    match error {
        DriverError::Status(status) => Error::Sdk { operation, status },
        DriverError::Contract(kind) => Error::ContractViolation { operation, kind },
        DriverError::Allocation { requested } => Error::AllocationFailed { requested },
        DriverError::OrphanedHandle(source) => Error::OpenFailedWithHandle {
            operation,
            source: Box::new(map_driver_error(operation, *source)),
        },
    }
}

fn validate_device_count(operation: &'static str, count: u32) -> Result<(), Error> {
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
    validate_device_count("MV3D_LP_GetDeviceList", reported)?;
    let reported = usize::try_from(reported).unwrap_or(usize::MAX);
    let doubled = current.saturating_mul(2).min(MAX_DEVICE_COUNT);
    let next = reported.max(doubled).max(current.saturating_add(1));
    if next > MAX_DEVICE_COUNT {
        return Err(Error::ContractViolation {
            operation: "MV3D_LP_GetDeviceList",
            kind: ContractViolation::DeviceCountExceedsLimit {
                reported: u32::try_from(next).unwrap_or(u32::MAX),
                limit: MAX_DEVICE_COUNT,
            },
        });
    }
    Ok(next)
}

fn validated_c_string(
    operation: &'static str,
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
