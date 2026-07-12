use std::marker::PhantomData;
use std::rc::Rc;
use std::time::Duration;

use crate::{
    CommandKey, Error, FileTransfer, InputViolation, OwnedFrame, ParamKey, Parameter,
    ParameterValue, Result, SdkText,
};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CameraState {
    Open,
    Measuring,
    Faulted,
    Transferring,
}

/// A borrowed device session. The camera cannot outlive its owning [`crate::Sdk`].
///
/// It is intentionally neither `Send` nor `Sync`, and no native handle is
/// exposed. All SDK calls remain serialized inside the private crate.
pub struct Camera<'sdk> {
    inner: mv3d_lp_internal::Camera<'sdk>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'sdk> Camera<'sdk> {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::Camera<'sdk>) -> Self {
        Self {
            inner,
            _not_send_or_sync: PhantomData,
        }
    }

    #[must_use]
    pub fn state(&self) -> CameraState {
        match self.inner.state() {
            mv3d_lp_internal::CameraState::Open => CameraState::Open,
            mv3d_lp_internal::CameraState::Measuring => CameraState::Measuring,
            mv3d_lp_internal::CameraState::Faulted => CameraState::Faulted,
            mv3d_lp_internal::CameraState::Transferring => CameraState::Transferring,
        }
    }

    /// Starts acquisition and returns an exclusive measurement guard.
    ///
    /// A failed start leaves the camera faulted so only cleanup remains valid.
    pub fn start(&mut self) -> Result<Measurement<'_>> {
        self.inner
            .start()
            .map(Measurement::from_internal)
            .map_err(Error::from)
    }

    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer().map_err(Error::from)
    }

    pub fn get_parameter(&mut self, key: &ParamKey) -> Result<Parameter> {
        self.inner
            .get_parameter(key.as_bytes())
            .map_err(Error::from)
            .and_then(parameter_from_internal)
    }

    pub fn set_parameter(&mut self, key: &ParamKey, value: ParameterValue) -> Result<()> {
        let internal_value = parameter_value_to_internal(value);
        self.inner
            .set_parameter(key.as_bytes(), &internal_value)
            .map_err(Error::from)
    }

    pub fn execute(&mut self, key: &CommandKey) -> Result<()> {
        self.inner.execute(key.as_bytes()).map_err(Error::from)
    }

    /// Starts copying a file from the camera to the host.
    ///
    /// Names are passed as original narrow-string bytes because the vendor SDK
    /// does not document their encoding.
    pub fn download_file<'camera>(
        &'camera mut self,
        device_file_name: &[u8],
        local_file_name: &[u8],
    ) -> Result<FileTransfer<'camera, 'sdk>> {
        self.inner
            .download_file(device_file_name, local_file_name)
            .map(|inner| FileTransfer {
                inner,
                _not_send_or_sync: PhantomData,
            })
            .map_err(Error::from)
    }

    /// Starts copying a host file into the camera.
    pub fn upload_file<'camera>(
        &'camera mut self,
        local_file_name: &[u8],
        device_file_name: &[u8],
    ) -> Result<FileTransfer<'camera, 'sdk>> {
        self.inner
            .upload_file(local_file_name, device_file_name)
            .map(|inner| FileTransfer {
                inner,
                _not_send_or_sync: PhantomData,
            })
            .map_err(Error::from)
    }

    /// Resumes polling after a previous transfer guard was dropped.
    pub fn active_file_transfer(&mut self) -> Option<FileTransfer<'_, 'sdk>> {
        self.inner.active_file_transfer().map(|inner| FileTransfer {
            inner,
            _not_send_or_sync: PhantomData,
        })
    }

    pub fn close(self) -> Result<()> {
        self.inner.close().map_err(|error| Error::DeviceCleanup {
            stop: error.stop.map(|error| Box::new(Error::from(*error))),
            close: error.close.map(|error| Box::new(Error::from(*error))),
        })
    }
}

/// An active pull-acquisition session borrowing its camera exclusively.
///
/// Dropping this guard makes a best-effort attempt to stop measurement. Call
/// [`Measurement::stop`] to observe a stop error explicitly.
#[must_use = "dropping Measurement stops acquisition"]
pub struct Measurement<'camera> {
    inner: mv3d_lp_internal::Measurement<'camera>,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'camera> Measurement<'camera> {
    fn from_internal(inner: mv3d_lp_internal::Measurement<'camera>) -> Self {
        Self {
            inner,
            _not_send_or_sync: PhantomData,
        }
    }

    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger().map_err(Error::from)
    }

    pub fn clear_buffer(&mut self) -> Result<()> {
        self.inner.clear_buffer().map_err(Error::from)
    }

    /// Waits up to `timeout` for one frame and copies every returned SDK payload.
    ///
    /// A zero duration performs a non-blocking poll. Non-zero sub-millisecond
    /// durations round up to one millisecond, and the SDK's infinite-wait
    /// sentinel is rejected. A completed wait with no frame is reported as
    /// [`crate::StatusCode::NO_DATA`] through [`crate::Error::Sdk`].
    pub fn get_image(&mut self, timeout: Duration) -> Result<OwnedFrame> {
        let timeout_ms = timeout_millis(timeout)?;
        self.inner
            .get_image(timeout_ms)
            .map(OwnedFrame::from_internal)
            .map_err(Error::from)
    }

    pub fn get_parameter(&mut self, key: &ParamKey) -> Result<Parameter> {
        self.inner
            .get_parameter(key.as_bytes())
            .map_err(Error::from)
            .and_then(parameter_from_internal)
    }

    pub fn set_parameter(&mut self, key: &ParamKey, value: ParameterValue) -> Result<()> {
        let internal_value = parameter_value_to_internal(value);
        self.inner
            .set_parameter(key.as_bytes(), &internal_value)
            .map_err(Error::from)
    }

    pub fn execute(&mut self, key: &CommandKey) -> Result<()> {
        self.inner.execute(key.as_bytes()).map_err(Error::from)
    }

    pub fn stop(self) -> Result<()> {
        self.inner.stop().map_err(Error::from)
    }
}

fn timeout_millis(timeout: Duration) -> Result<u32> {
    const NANOS_PER_MILLI: u128 = 1_000_000;
    const MAXIMUM_MILLIS: u32 = u32::MAX - 1;

    let millis = timeout.as_nanos().div_ceil(NANOS_PER_MILLI);
    if millis > u128::from(MAXIMUM_MILLIS) {
        return Err(Error::InvalidInput {
            field: "timeout",
            violation: InputViolation::TimeoutTooLong {
                maximum_millis: MAXIMUM_MILLIS,
                actual_millis: millis,
            },
        });
    }
    Ok(millis as u32)
}

fn parameter_value_to_internal(value: ParameterValue) -> mv3d_lp_internal::ParameterValueRecord {
    match value {
        ParameterValue::Bool(value) => mv3d_lp_internal::ParameterValueRecord::Bool(value),
        ParameterValue::Integer(value) => mv3d_lp_internal::ParameterValueRecord::Integer(value),
        ParameterValue::Float(value) => mv3d_lp_internal::ParameterValueRecord::Float(value),
        ParameterValue::Enumeration(value) => {
            mv3d_lp_internal::ParameterValueRecord::Enumeration(value)
        }
        ParameterValue::String(value) => {
            mv3d_lp_internal::ParameterValueRecord::String(value.into_bytes())
        }
    }
}

fn parameter_from_internal(record: mv3d_lp_internal::ParameterRecord) -> Result<Parameter> {
    Ok(match record {
        mv3d_lp_internal::ParameterRecord::Bool(value) => Parameter::Bool(value),
        mv3d_lp_internal::ParameterRecord::Integer {
            value,
            minimum,
            maximum,
            increment,
        } => Parameter::Integer {
            value,
            min: minimum,
            max: maximum,
            increment,
        },
        mv3d_lp_internal::ParameterRecord::Float {
            value,
            minimum,
            maximum,
        } => Parameter::Float {
            value,
            min: minimum,
            max: maximum,
        },
        mv3d_lp_internal::ParameterRecord::Enumeration { value, supported } => {
            Parameter::Enumeration { value, supported }
        }
        mv3d_lp_internal::ParameterRecord::String {
            value,
            maximum_length,
        } => Parameter::String {
            value: SdkText::new(value)?,
            max_length: maximum_length,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{Error, InputViolation};

    use super::timeout_millis;

    #[test]
    fn timeout_conversion_is_finite_checked_and_rounds_up() {
        assert_eq!(timeout_millis(Duration::ZERO).unwrap(), 0);
        assert_eq!(timeout_millis(Duration::from_nanos(1)).unwrap(), 1);
        assert_eq!(timeout_millis(Duration::from_millis(37)).unwrap(), 37);
        assert_eq!(
            timeout_millis(Duration::from_millis(u64::from(u32::MAX - 1))).unwrap(),
            u32::MAX - 1
        );
        assert!(matches!(
            timeout_millis(
                Duration::from_millis(u64::from(u32::MAX - 1)) + Duration::from_nanos(1)
            ),
            Err(Error::InvalidInput {
                violation: InputViolation::TimeoutTooLong { .. },
                ..
            })
        ));
        assert!(matches!(
            timeout_millis(Duration::from_millis(u64::from(u32::MAX))),
            Err(Error::InvalidInput {
                violation: InputViolation::TimeoutTooLong { .. },
                ..
            })
        ));
    }
}
