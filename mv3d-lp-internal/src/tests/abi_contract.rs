use std::mem::{align_of, offset_of, size_of};

use crate::bindings::{MV3D_LP_DEVICE_INFO, MV3D_LP_IMAGE_DATA, MV3D_LP_PARAM};

#[test]
fn critical_x64_layouts_match_the_audited_baseline() {
    assert_eq!(size_of::<MV3D_LP_DEVICE_INFO>(), 268);
    assert_eq!(align_of::<MV3D_LP_DEVICE_INFO>(), 4);
    assert_eq!(size_of::<MV3D_LP_IMAGE_DATA>(), 112);
    assert_eq!(align_of::<MV3D_LP_IMAGE_DATA>(), 8);
    assert_eq!(size_of::<MV3D_LP_PARAM>(), 288);
    assert_eq!(align_of::<MV3D_LP_PARAM>(), 8);
    assert_eq!(offset_of!(MV3D_LP_PARAM, ParamInfo), 8);
    assert_eq!(offset_of!(MV3D_LP_PARAM, nReserved), 272);
}
