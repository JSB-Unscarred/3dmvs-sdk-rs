use std::net::Ipv4Addr;

use crate::error::{Error, InvalidInput};
use crate::parameter::ParameterValueRecord;

use super::mock_driver::{MockDriver, active_runtime};

// 验证非法参数 key 在 driver 前被拒绝，防止 NUL 截断或空 key 进入 SDK。
#[test]
fn invalid_parameter_keys_never_reach_the_driver() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();
    let before = mock.logs();

    assert!(matches!(
        device.execute(b""),
        Err(Error::InvalidInput {
            kind: InvalidInput::Empty,
            ..
        })
    ));
    assert_eq!(mock.logs(), before);
}

// 验证过长或含 NUL 的参数字符串本地拒绝，防止写入固定 C 字符串缓冲区失败。
#[test]
fn oversized_or_nul_parameter_strings_are_rejected_locally() {
    let mock = MockDriver::new();
    let (runtime, _) = active_runtime(&mock);
    let mut device = runtime.open_by_ip(Ipv4Addr::LOCALHOST).unwrap();

    let oversized = ParameterValueRecord::String(vec![b'x'; 256]);
    assert!(matches!(
        device.set_parameter(b"Name", &oversized),
        Err(Error::InvalidInput {
            kind: InvalidInput::TooLong { .. },
            ..
        })
    ));
    let with_nul = ParameterValueRecord::String(b"a\0b".to_vec());
    assert!(matches!(
        device.set_parameter(b"Name", &with_nul),
        Err(Error::InvalidInput {
            kind: InvalidInput::InteriorNul,
            ..
        })
    ));
}
