use std::marker::PhantomData;
use std::rc::Rc;

use crate::driver::Handle;
use crate::error::{Error, InvalidInput};
use crate::parameter::{ParameterRecord, ParameterValueRecord};
use crate::runtime::Runtime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CameraState {
    Open,
    Measuring,
    Faulted,
}

impl CameraState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Measuring => "measuring",
            Self::Faulted => "faulted",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CleanupError {
    pub stop: Option<Error>,
    pub close: Option<Error>,
}

pub struct Camera<'runtime> {
    runtime: &'runtime Runtime,
    handle: Option<Handle>,
    state: CameraState,
    _not_send_or_sync: PhantomData<Rc<()>>,
}

impl<'runtime> Camera<'runtime> {
    pub(crate) fn new(runtime: &'runtime Runtime, handle: Handle) -> Self {
        Self {
            runtime,
            handle: Some(handle),
            state: CameraState::Open,
            _not_send_or_sync: PhantomData,
        }
    }

    pub fn state(&self) -> CameraState {
        self.state
    }

    pub fn start(&mut self) -> Result<(), Error> {
        self.require_state("MV3D_LP_StartMeasure", &[CameraState::Open])?;
        let result = self
            .runtime
            .call("MV3D_LP_StartMeasure", |driver| driver.start(self.handle()));
        match result {
            Ok(()) => {
                self.state = CameraState::Measuring;
                Ok(())
            }
            Err(error) => {
                self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    pub fn stop(&mut self) -> Result<(), Error> {
        self.require_state("MV3D_LP_StopMeasure", &[CameraState::Measuring])?;
        let result = self
            .runtime
            .call("MV3D_LP_StopMeasure", |driver| driver.stop(self.handle()));
        match result {
            Ok(()) => {
                self.state = CameraState::Open;
                Ok(())
            }
            Err(error) => {
                self.state = CameraState::Faulted;
                Err(error)
            }
        }
    }

    pub fn soft_trigger(&mut self) -> Result<(), Error> {
        self.require_state("MV3D_LP_SoftTrigger", &[CameraState::Measuring])?;
        self.runtime.call("MV3D_LP_SoftTrigger", |driver| {
            driver.soft_trigger(self.handle())
        })
    }

    pub fn clear_buffer(&mut self) -> Result<(), Error> {
        self.require_state(
            "MV3D_LP_ClearDataBuffer",
            &[CameraState::Open, CameraState::Measuring],
        )?;
        self.runtime.call("MV3D_LP_ClearDataBuffer", |driver| {
            driver.clear_buffer(self.handle())
        })
    }

    pub fn get_parameter(&mut self, key: &[u8]) -> Result<ParameterRecord, Error> {
        self.require_usable("MV3D_LP_GetParam")?;
        let key = Runtime::parameter_key("MV3D_LP_GetParam", key)?;
        self.runtime.call("MV3D_LP_GetParam", |driver| {
            driver.get_parameter(self.handle(), &key)
        })
    }

    pub fn set_parameter(&mut self, key: &[u8], value: &ParameterValueRecord) -> Result<(), Error> {
        self.require_usable("MV3D_LP_SetParam")?;
        if let ParameterValueRecord::String(value) = value {
            if value.len() > 255 {
                return Err(Error::InvalidInput {
                    operation: "MV3D_LP_SetParam",
                    kind: InvalidInput::TooLong {
                        actual: value.len(),
                        maximum: 255,
                    },
                });
            }
            if value.contains(&0) {
                return Err(Error::InvalidInput {
                    operation: "MV3D_LP_SetParam",
                    kind: InvalidInput::InteriorNul,
                });
            }
        }
        let key = Runtime::parameter_key("MV3D_LP_SetParam", key)?;
        self.runtime.call("MV3D_LP_SetParam", |driver| {
            driver.set_parameter(self.handle(), &key, value)
        })
    }

    pub fn execute(&mut self, key: &[u8]) -> Result<(), Error> {
        self.require_usable("MV3D_LP_Execute")?;
        let key = Runtime::parameter_key("MV3D_LP_Execute", key)?;
        self.runtime.call("MV3D_LP_Execute", |driver| {
            driver.execute(self.handle(), &key)
        })
    }

    pub fn close(mut self) -> Result<(), CleanupError> {
        self.cleanup()
    }

    fn cleanup(&mut self) -> Result<(), CleanupError> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };

        let stop = if matches!(self.state, CameraState::Measuring | CameraState::Faulted) {
            self.runtime
                .cleanup_call("MV3D_LP_StopMeasure", |driver| driver.stop(handle))
                .err()
        } else {
            None
        };
        let close = self
            .runtime
            .cleanup_call("MV3D_LP_CloseDevice", |driver| driver.close(handle))
            .err();
        self.runtime.record_close_result(close.is_none());

        if stop.is_none() && close.is_none() {
            Ok(())
        } else {
            Err(CleanupError { stop, close })
        }
    }

    fn handle(&self) -> Handle {
        self.handle.expect("a live camera always has a handle")
    }

    fn require_usable(&self, operation: &'static str) -> Result<(), Error> {
        self.require_state(operation, &[CameraState::Open, CameraState::Measuring])
    }

    fn require_state(&self, operation: &'static str, allowed: &[CameraState]) -> Result<(), Error> {
        if allowed.contains(&self.state) {
            Ok(())
        } else {
            Err(Error::InvalidState {
                operation,
                state: self.state.as_str(),
            })
        }
    }
}

impl Drop for Camera<'_> {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}
