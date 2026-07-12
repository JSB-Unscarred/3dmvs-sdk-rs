use std::net::Ipv4Addr;

use crate::error::{Error, InvalidInput};
use crate::parameter::ParameterValueRecord;

use super::mock_driver::{MockDriver, active_runtime};

#[test]
fn invalid_parameter_keys_never_reach_the_driver() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let before = mock.logs();

    assert!(matches!(
        camera.execute(b""),
        Err(Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        })
    ));
    assert_eq!(mock.logs(), before);
}

#[test]
fn oversized_or_nul_parameter_strings_are_rejected_locally() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut camera = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let oversized = ParameterValueRecord::String(vec![b'x'; 256]);
    assert!(matches!(
        camera.set_parameter(b"Name", &oversized),
        Err(Error::InvalidInput {
            kind: InvalidInput::TooLong { .. },
            ..
        })
    ));
    let with_nul = ParameterValueRecord::String(b"a\0b".to_vec());
    assert!(matches!(
        camera.set_parameter(b"Name", &with_nul),
        Err(Error::InvalidInput {
            kind: InvalidInput::InteriorNul,
            ..
        })
    ));
}
