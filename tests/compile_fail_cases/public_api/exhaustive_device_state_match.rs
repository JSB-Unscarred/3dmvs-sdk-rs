use mv3d_lp::DeviceState;

fn state_name(state: DeviceState) -> &'static str {
    match state {
        DeviceState::Open => "open",
        DeviceState::Measuring => "measuring",
        DeviceState::CallbackMeasuring => "callback measuring",
        DeviceState::Faulted => "faulted",
        DeviceState::Transferring => "transferring",
    }
}

fn main() {
    let _ = state_name(DeviceState::Open);
}
