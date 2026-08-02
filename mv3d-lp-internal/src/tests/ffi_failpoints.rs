use std::collections::HashSet;
use std::ffi::CString;
#[cfg(feature = "display-windows")]
use std::num::NonZeroIsize;
use std::sync::Arc;

use crate::callback::{
    CallbackDelivery, CallbackRegistration, ExceptionCallbackSink, FrameCallbackSink,
};
use crate::device::{IpConfigRaw, IpConfiguration};
#[cfg(feature = "display-windows")]
use crate::display::DisplayRangeRecord;
use crate::driver::{Driver, DriverError, DriverResult};
use crate::error::{Error, Operation};
use crate::frame::{ImageFileFormatRecord, ImageInput, ImageTypeRecord};
use crate::parameter::ParameterValueRecord;

use super::mock_driver::{FfiOp, MockDriver, active_runtime, mock_handle};

const INJECTED_STATUS: i32 = 0xE005_0001_u32 as i32;

#[test]
fn driver_failpoint_ledger_is_complete_and_has_unique_vendor_names() {
    #[cfg(not(feature = "display-windows"))]
    assert_eq!(FfiOp::ALL.len(), 27);
    #[cfg(feature = "display-windows")]
    assert_eq!(FfiOp::ALL.len(), 28);

    let names = FfiOp::ALL
        .iter()
        .map(|operation| operation.sdk_name())
        .collect::<HashSet<_>>();
    assert_eq!(names.len(), FfiOp::ALL.len());
    assert!(names.iter().all(|name| name.starts_with("MV3D_LP_")));

    let source = include_str!("../driver.rs");
    let trait_body = source
        .split_once("pub(crate) trait Driver: Sync {")
        .expect("Driver trait declaration exists")
        .1
        .split_once("\n}")
        .expect("Driver trait has a closing brace")
        .0;
    let declared = trait_body
        .lines()
        .filter_map(|line| line.trim_start().strip_prefix("fn "))
        .filter_map(|signature| signature.split_once('(').map(|(name, _)| name))
        .filter(|name| cfg!(feature = "display-windows") || *name != "display_image")
        .collect::<HashSet<_>>();
    let covered = FfiOp::ALL
        .iter()
        .map(|operation| operation.driver_method())
        .collect::<HashSet<_>>();
    assert_eq!(covered, declared, "every Driver method needs a failpoint");
}

#[test]
fn every_driver_operation_consumes_the_shared_failure_injector() {
    for &operation in FfiOp::ALL {
        let mock = MockDriver::new();
        let injected = DriverError::Status(INJECTED_STATUS);
        mock.fail_next(operation, injected.clone());

        assert_eq!(exercise(&mock, operation), Err(injected));
        assert_eq!(mock.operations(), [operation]);
        assert_eq!(mock.in_flight(), 0);
        assert_eq!(mock.maximum_in_flight(), 1);
        mock.assert_no_pending_failures();
    }
}

#[test]
fn previously_missing_control_failures_are_observable_without_poisoning_the_runtime() {
    let mock = MockDriver::new();
    mock.configure_open_by_serial(None, Err(DriverError::Status(INJECTED_STATUS)));
    mock.push_set_ip_config(Err(DriverError::Status(INJECTED_STATUS)));
    let (runtime, _) = active_runtime(&mock);

    assert!(matches!(
        runtime.open_by_serial(b"SERIAL"),
        Err(Error::Sdk {
            operation: Operation::OpenDeviceBySn,
            status: INJECTED_STATUS,
        })
    ));
    assert!(matches!(
        runtime.set_ip_config(b"SERIAL", &IpConfiguration::Dhcp),
        Err(Error::Sdk {
            operation: Operation::SetIpConfig,
            status: INJECTED_STATUS,
        })
    ));

    let mut device = runtime.open_by_ip("192.0.2.1".parse().unwrap()).unwrap();
    mock.push_set_parameter(Err(DriverError::Status(INJECTED_STATUS)));
    assert!(matches!(
        device.set_parameter(b"Enabled", &ParameterValueRecord::Bool(true)),
        Err(Error::Sdk {
            operation: Operation::SetParam,
            status: INJECTED_STATUS,
        })
    ));
    mock.push_execute(Err(DriverError::Status(INJECTED_STATUS)));
    assert!(matches!(
        device.execute(b"Reset"),
        Err(Error::Sdk {
            operation: Operation::Execute,
            status: INJECTED_STATUS,
        })
    ));

    device.close().unwrap();
    runtime.shutdown().unwrap();
    assert_eq!(
        mock.operations(),
        [
            FfiOp::GetVersion,
            FfiOp::Initialize,
            FfiOp::OpenDeviceBySerial,
            FfiOp::SetIpConfig,
            FfiOp::OpenDeviceByIp,
            FfiOp::SetParameter,
            FfiOp::Execute,
            FfiOp::CloseDevice,
            FfiOp::Finalize,
        ]
    );
}

fn exercise(mock: &MockDriver, operation: FfiOp) -> DriverResult<()> {
    let selector = CString::new("value").unwrap();
    let config = IpConfigRaw::from(&IpConfiguration::Dhcp);
    let handle = mock_handle(0x100);
    let data = [0_u8];
    let image = ImageInput {
        image_type: ImageTypeRecord::from_bits(0x0108_0001),
        width: 1,
        height: 1,
        data: &data,
        intensity_data: None,
        exposure_timestamps: None,
        frame_number: 0,
        device_timestamp: 0,
        valid: true,
        x_scale: 1.0,
        y_scale: 1.0,
        z_scale: 1.0,
        x_offset: 0,
        y_offset: 0,
        z_offset: 0,
    };

    match operation {
        FfiOp::GetVersion => mock.version().map(|_| ()),
        FfiOp::Initialize => mock.initialize(),
        FfiOp::Finalize => mock.finalize(),
        FfiOp::GetDeviceNumber => mock.device_number().map(|_| ()),
        FfiOp::GetDeviceList => mock.device_list(1).map(|_| ()),
        FfiOp::SetIpConfig => mock.set_ip_config(&selector, &config),
        FfiOp::OpenDeviceByIp => mock.open_by_ip(&selector, &mut None),
        FfiOp::OpenDeviceBySerial => mock.open_by_serial(&selector, &mut None),
        FfiOp::CloseDevice => mock.close(handle),
        FfiOp::StartMeasure => mock.start(handle),
        FfiOp::StopMeasure => mock.stop(handle),
        FfiOp::SoftTrigger => mock.soft_trigger(handle),
        FfiOp::ClearDataBuffer => mock.clear_buffer(handle),
        FfiOp::GetImage => mock.get_image(handle, 1).map(|_| ()),
        FfiOp::RegisterImageCallback => {
            let sink: FrameCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
            let registration = CallbackRegistration::image(sink).unwrap();
            mock.register_image_callback(handle, registration.cookie())
        }
        FfiOp::RegisterExceptionCallback => {
            let sink: ExceptionCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
            let registration = CallbackRegistration::exception(sink).unwrap();
            mock.register_exception_callback(handle, registration.cookie())
        }
        FfiOp::GetParameter => mock.get_parameter(handle, &selector).map(|_| ()),
        FfiOp::SetParameter => {
            mock.set_parameter(handle, &selector, &ParameterValueRecord::Bool(true))
        }
        FfiOp::Execute => mock.execute(handle, &selector),
        FfiOp::FileAccessRead => mock.file_access_read(handle, &selector, &selector),
        FfiOp::FileAccessWrite => mock.file_access_write(handle, &selector, &selector),
        FfiOp::GetFileAccessProgress => mock.file_access_progress(handle).map(|_| ()),
        FfiOp::MapDepthToPointCloud => mock.map_depth_to_point_cloud(image).map(|_| ()),
        FfiOp::MapDepthToPointCloudRound => mock
            .map_depth_to_point_cloud_round(std::slice::from_ref(&image))
            .map(|_| ()),
        FfiOp::ImageConvert => mock
            .convert_image(image, ImageTypeRecord::from_bits(0x0108_0001))
            .map(|_| ()),
        FfiOp::DepthMosaic => mock.mosaic_depth(std::slice::from_ref(&image)).map(|_| ()),
        FfiOp::SaveImage => mock.save_image(image, ImageFileFormatRecord::Bmp, &selector),
        #[cfg(feature = "display-windows")]
        FfiOp::DisplayImage => mock.display_image(
            image,
            NonZeroIsize::new(1).unwrap(),
            DisplayRangeRecord::Auto,
        ),
    }
}
