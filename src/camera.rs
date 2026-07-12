use std::marker::PhantomData;
use std::rc::Rc;

use crate::{CommandKey, Error, ParamKey, Parameter, ParameterValue, Result, SdkText};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CameraState {
    Open,
    Measuring,
    Faulted,
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
        }
    }

    pub fn start(&mut self) -> Result<()> {
        self.inner.start().map_err(Error::from)
    }

    pub fn stop(&mut self) -> Result<()> {
        self.inner.stop().map_err(Error::from)
    }

    pub fn soft_trigger(&mut self) -> Result<()> {
        self.inner.soft_trigger().map_err(Error::from)
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
        let internal_value = match value {
            ParameterValue::Bool(value) => mv3d_lp_internal::ParameterValueRecord::Bool(value),
            ParameterValue::Integer(value) => {
                mv3d_lp_internal::ParameterValueRecord::Integer(value)
            }
            ParameterValue::Float(value) => mv3d_lp_internal::ParameterValueRecord::Float(value),
            ParameterValue::Enumeration(value) => {
                mv3d_lp_internal::ParameterValueRecord::Enumeration(value)
            }
            ParameterValue::String(value) => {
                mv3d_lp_internal::ParameterValueRecord::String(value.into_bytes())
            }
        };
        self.inner
            .set_parameter(key.as_bytes(), &internal_value)
            .map_err(Error::from)
    }

    pub fn execute(&mut self, key: &CommandKey) -> Result<()> {
        self.inner.execute(key.as_bytes()).map_err(Error::from)
    }

    pub fn close(self) -> Result<()> {
        self.inner.close().map_err(|error| Error::DeviceCleanup {
            stop: error.stop.map(|error| Box::new(Error::from(error))),
            close: error.close.map(|error| Box::new(Error::from(error))),
        })
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
