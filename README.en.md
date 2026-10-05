# mv3d-lp

[中文](README.md)

Unofficial safe Rust wrapper for the Hikrobot 3D laser profiler SDK (LPSDK). The raw FFI lives in
`mv3d-lp-sys` and is re-exported as `mv3d_lp::sys`.

## Requirements

- Only `x86_64-pc-windows-msvc` is supported.
- Building does not need 3DMVS installed.
- At run time, 3DMVS (LPSDK 1.3.3.3 or later) must be installed and the directory of `Mv3dLp.dll` must be on
  `PATH` (the 3DMVS installer adds it by default). If the DLL cannot be found, the program exits with
  `0xC0000135` before `main` runs and prints nothing.
- LPSDK creates a `Mv3dLpLog` log directory in the working directory of the process.

```toml
[dependencies]
mv3d-lp = "0.1"
```

## Example

```rust,no_run
use std::net::Ipv4Addr;
use std::time::Duration;

use mv3d_lp::Sdk;

fn main() -> mv3d_lp::Result<()> {
    let sdk = Sdk::new()?;
    let mut device = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;

    let grabbing = device.start_grabbing()?;
    let image = grabbing.get_image(Some(Duration::from_secs(1)))?;
    println!("{}x{}, {} bytes", image.width, image.height, image.data.len());
    Ok(())
}
```

Complete examples are in [`examples/`](examples): device enumeration (`enumerate_devices`), pull grabbing that
saves a point cloud (`grab_pull`) and callback grabbing (`grab_callback`).

## Usage notes

- **Session**: a process has a single SDK session; `Sdk::new` and `clone` both return it. The SDK is finalized
  once the `Sdk` and every `Device` are dropped, after which `Sdk::new` returns `Error::Finalized`. The image
  processing interfaces are methods of `Sdk`, so a worker thread can use its own clone.
- **Grabbing**: the guards returned by `start_grabbing` and `start_grabbing_with` borrow the device mutably
  and stop grabbing when dropped. Parameters can still be read and written, and soft triggers sent, while
  grabbing.
- **Image callbacks cannot be unregistered**: once a device has registered an image callback, it can only grab
  through callbacks until it is closed, and `start_grabbing` returns `Error::ImageCallbackRegistered`. The same
  holds when `start_grabbing_with` registers the callback but fails to start grabbing.
- **Callbacks**: they run on an SDK thread, and a panic inside one aborts the process. Do not close the device
  or stop grabbing inside a callback; hand that over to the thread that owns the device through a channel.
- **Images**: every `Image` from grabbing or image processing is an owned copy and can be kept freely. When
  used as processing input, the data length must match the width, height and format, otherwise
  `Error::InvalidInput` is returned.
- **File transfer**: `download_file` and `upload_file` block until the transfer finishes.
- **Cleanup**: `Drop` ignores cleanup errors. Call `Device::close`, `Grabbing::stop` or `CallbackGrabbing::stop`
  to check them.
- **Strings**: parameter names and similar arguments are `&CStr`, for example `c"ExposureTime"`.

See [`docs::architecture`](https://docs.rs/mv3d-lp/latest/mv3d_lp/docs/architecture/index.html) for details
(the API documentation is written in Chinese).

## Wrapped interfaces

Based on `Mv3dLpApi.h` and `Mv3dLpImgProc.h` of LPSDK 1.3.3.3. A failed SDK call returns
`Error::Sdk { function, code }`, where `code` is an `ErrorCode`.

| SDK interface | Rust interface | Notes |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version() -> &'static CStr` | No initialization needed |
| `MV3D_LP_Initialize` | `Sdk::new() -> Result<Sdk>` | Initializes on the first call |
| `MV3D_LP_Finalize` | Called when the `Sdk` and every `Device` are dropped |  |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count(&self) -> Result<u32>` |  |
| `MV3D_LP_GetDeviceList` | `Sdk::devices(&self) -> Result<Vec<DeviceInfo>>` |  |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip(&self, Ipv4Addr) -> Result<Device>` |  |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial(&self, &CStr) -> Result<Device>` | Accepts `DeviceInfo::serial_number()` directly |
| `MV3D_LP_CloseDevice` | `Device::close(self) -> Result<()>`, `Drop` |  |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config(&self, &CStr, IpConfig) -> Result<()>` | The device is selected by serial number |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::register_exception_callback(&mut self, F) -> Result<()>` | `F: Fn(ExceptionInfo<'_>) + Send + Sync + 'static` |
| `MV3D_LP_StartMeasure` | `Device::start_grabbing(&mut self) -> Result<Grabbing<'_>>`, `start_grabbing_with` |  |
| `MV3D_LP_StopMeasure` | `Grabbing::stop(self)`, `CallbackGrabbing::stop(self)`, `Drop` of the guards |  |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger(&self) -> Result<()>` |  |
| `MV3D_LP_GetImage` | `Grabbing::get_image(&self, Option<Duration>) -> Result<Image>` | `None` waits forever |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_grabbing_with(&mut self, F) -> Result<CallbackGrabbing<'_>>` | `F: Fn(Image) + Send + Sync + 'static` |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer(&self) -> Result<()>` |  |
| `MV3D_LP_GetParam` | `Device::get_parameter(&self, &CStr) -> Result<Parameter>` |  |
| `MV3D_LP_SetParam` | `Device::set_parameter(&self, &CStr, ParameterValue<'_>) -> Result<()>` | Strings are limited to 255 bytes |
| `MV3D_LP_Execute` | `Device::execute_command(&self, &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessRead` | `Device::download_file(&self, device_file: &CStr, local_file: &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file(&self, local_file: &CStr, device_file: &CStr) -> Result<()>` |  |
| `MV3D_LP_GetFileAccessProgress` | Not wrapped | The transfer calls block, so progress cannot be queried meanwhile |
| `MV3D_LP_MapDepthToPointCloud` | `Sdk::depth_to_point_cloud(&self, &Image) -> Result<Image>` |  |
| `MV3D_LP_MapDepthToPointCloudRound` | `Sdk::depth_to_round_point_cloud(&self, &[Image]) -> Result<Image>` | At most 8 images |
| `MV3D_LP_ImageConvert` | `Sdk::convert_image(&self, &Image, ImageType) -> Result<Image>` |  |
| `MV3D_LP_DepthMosaic` | `Sdk::mosaic_depth(&self, &[Image]) -> Result<Image>` | At most 8 images |
| `MV3D_LP_SaveImage` | `Sdk::save_image(&self, &Image, ImageFileFormat, &CStr) -> Result<()>` |  |
| `MV3D_LP_DisplayImage` | `Sdk::display_image(&self, &Image, &W, DisplayRange) -> Result<()>` | `W: HasWindowHandle`, needs a Win32 window |

The interfaces the headers mark as deprecated (`GetDeviceIP`, `GetDeviceSN`, `GetProfile`, `GetBatchProfile`,
`GetIntensityData`, `RegisterProfileCallBack` and `RegisterBatchProfileCallBack`) are not wrapped; the same data
is available from `DeviceInfo` and `Image`.

## Structures

| SDK structure | Rust type | Notes |
| --- | --- | --- |
| `MV3D_LP_DEVICE_INFO` | `DeviceInfo` | Strings are `&CStr`, IPs are `Option<Ipv4Addr>`, the MAC is `[u8; 6]` |
| `MV3D_LP_IP_CONFIG` | `IpConfig` | `Static`, `Dhcp`, `LinkLocal` |
| `MV3D_LP_IMAGE_DATA` | `Image`, `ImageCalibration` |  |
| `MV3D_LP_PARAM` | `Parameter`, `ParameterValue<'_>` | An unknown type is read as `Parameter::Other` |
| `MV3D_LP_EXCEPTION_INFO` | `ExceptionInfo<'_>`, `ExceptionKind` | Only valid during the callback |

## License

MIT. Use of 3DMVS and LPSDK is subject to Hikrobot's license terms; this project does not redistribute any SDK
files.
