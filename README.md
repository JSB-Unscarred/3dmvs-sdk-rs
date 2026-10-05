# mv3d-lp

[English](README.en.md)

海康机器人（Hikrobot）3D 激光轮廓传感器 SDK（LPSDK）的安全 Rust 封装（非官方）。原始 FFI 在
`mv3d-lp-sys` 中，也可以经 `mv3d_lp::sys` 访问。

## 环境

- 只支持 `x86_64-pc-windows-msvc`。
- 编译不需要安装 3DMVS。
- 运行时需要安装 3DMVS（LPSDK 1.3.3.3 或更新版本），并且 `Mv3dLp.dll` 所在目录在 `PATH` 中（3DMVS 安装程序默认会添加）。
  找不到该 DLL 时，程序在进入 `main` 之前就以 `0xC0000135` 退出，不输出任何信息。
- LPSDK 会在进程的工作目录下创建 `Mv3dLpLog` 日志目录。

```toml
[dependencies]
mv3d-lp = "0.1"
```

## 示例

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

[`examples/`](examples) 中有完整示例：枚举设备（`enumerate_devices`）、主动取图并保存点云（`grab_pull`）与
callback 取图（`grab_callback`）。

## 使用须知

- **会话**：一个进程只有一个 SDK 会话，`Sdk::new` 与 `clone` 得到的都是它。`Sdk` 与所有 `Device`
  都释放后 SDK 反初始化，此后 `Sdk::new` 返回 `Error::Finalized`。图像处理接口在 `Sdk` 上，工作线程可以
  `clone` 一份使用。
- **取流**：`start_grabbing` 与 `start_grabbing_with` 返回的守卫可变借用设备，释放时停止取流；
  取流期间仍可读写参数、发送软触发。
- **Image callback 不能注销**：设备注册过 image callback 后，关闭前只能用 callback 取图，`start_grabbing`
  返回 `Error::ImageCallbackRegistered`。`start_grabbing_with` 注册成功但开始取流失败时也是如此。
- **Callback**：在 SDK 的线程中运行，其中的 panic 会终止进程。不要在 callback 里关闭设备或停止取流，
  应通过 channel 交给持有设备的线程。
- **图像**：取流与图像处理得到的 `Image` 都是复制出来的数据，可以随意保留。作为处理输入时，数据长度必须与
  宽高、格式对应，否则返回 `Error::InvalidInput`。
- **文件传输**：`download_file` 与 `upload_file` 阻塞到传输结束。
- **清理**：`Drop` 忽略清理错误；需要检查时调用 `Device::close`、`Grabbing::stop` 或 `CallbackGrabbing::stop`。
- **字符串**：参数名等参数使用 `&CStr`，例如 `c"ExposureTime"`。

细节见 [`docs::architecture`](https://docs.rs/mv3d-lp/latest/mv3d_lp/docs/architecture/index.html)。

## 已封装的接口

以 LPSDK 1.3.3.3 的 `Mv3dLpApi.h` 与 `Mv3dLpImgProc.h` 为准。SDK 调用失败时返回
`Error::Sdk { function, code }`，`code` 的类型是 `ErrorCode`。

| SDK 接口 | Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version() -> &'static CStr` | 无需初始化 |
| `MV3D_LP_Initialize` | `Sdk::new() -> Result<Sdk>` | 首次调用时初始化 |
| `MV3D_LP_Finalize` | `Sdk` 与所有 `Device` 释放时自动调用 |  |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count(&self) -> Result<u32>` |  |
| `MV3D_LP_GetDeviceList` | `Sdk::devices(&self) -> Result<Vec<DeviceInfo>>` |  |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip(&self, Ipv4Addr) -> Result<Device>` |  |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial(&self, &CStr) -> Result<Device>` | 可直接传入 `DeviceInfo::serial_number()` |
| `MV3D_LP_CloseDevice` | `Device::close(self) -> Result<()>`、`Drop` |  |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config(&self, &CStr, IpConfig) -> Result<()>` | 按序列号指定设备 |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::register_exception_callback(&mut self, F) -> Result<()>` | `F: Fn(ExceptionInfo<'_>) + Send + Sync + 'static` |
| `MV3D_LP_StartMeasure` | `Device::start_grabbing(&mut self) -> Result<Grabbing<'_>>`、`start_grabbing_with` |  |
| `MV3D_LP_StopMeasure` | `Grabbing::stop(self)`、`CallbackGrabbing::stop(self)`、守卫的 `Drop` |  |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger(&self) -> Result<()>` |  |
| `MV3D_LP_GetImage` | `Grabbing::get_image(&self, Option<Duration>) -> Result<Image>` | `None` 表示无限等待 |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_grabbing_with(&mut self, F) -> Result<CallbackGrabbing<'_>>` | `F: Fn(Image) + Send + Sync + 'static` |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer(&self) -> Result<()>` |  |
| `MV3D_LP_GetParam` | `Device::get_parameter(&self, &CStr) -> Result<Parameter>` |  |
| `MV3D_LP_SetParam` | `Device::set_parameter(&self, &CStr, ParameterValue<'_>) -> Result<()>` | 字符串最多 255 字节 |
| `MV3D_LP_Execute` | `Device::execute_command(&self, &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessRead` | `Device::download_file(&self, device_file: &CStr, local_file: &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file(&self, local_file: &CStr, device_file: &CStr) -> Result<()>` |  |
| `MV3D_LP_GetFileAccessProgress` | 未封装 | 传输调用阻塞，期间无法查询 |
| `MV3D_LP_MapDepthToPointCloud` | `Sdk::depth_to_point_cloud(&self, &Image) -> Result<Image>` |  |
| `MV3D_LP_MapDepthToPointCloudRound` | `Sdk::depth_to_round_point_cloud(&self, &[Image]) -> Result<Image>` | 最多 8 张 |
| `MV3D_LP_ImageConvert` | `Sdk::convert_image(&self, &Image, ImageType) -> Result<Image>` |  |
| `MV3D_LP_DepthMosaic` | `Sdk::mosaic_depth(&self, &[Image]) -> Result<Image>` | 最多 8 张 |
| `MV3D_LP_SaveImage` | `Sdk::save_image(&self, &Image, ImageFileFormat, &CStr) -> Result<()>` |  |
| `MV3D_LP_DisplayImage` | `Sdk::display_image(&self, &Image, &W, DisplayRange) -> Result<()>` | `W: HasWindowHandle`，需要 Win32 窗口 |

头文件中已废弃的 `GetDeviceIP`、`GetDeviceSN`、`GetProfile`、`GetBatchProfile`、`GetIntensityData`、
`RegisterProfileCallBack` 与 `RegisterBatchProfileCallBack` 不封装，对应数据可以从 `DeviceInfo` 与 `Image` 取得。

## 结构体

| SDK 结构体 | Rust 类型 | 说明 |
| --- | --- | --- |
| `MV3D_LP_DEVICE_INFO` | `DeviceInfo` | 字符串为 `&CStr`，IP 为 `Option<Ipv4Addr>`，MAC 为 `[u8; 6]` |
| `MV3D_LP_IP_CONFIG` | `IpConfig` | `Static`、`Dhcp`、`LinkLocal` |
| `MV3D_LP_IMAGE_DATA` | `Image`、`ImageCalibration` |  |
| `MV3D_LP_PARAM` | `Parameter`、`ParameterValue<'_>` | 读到未知类型时为 `Parameter::Other` |
| `MV3D_LP_EXCEPTION_INFO` | `ExceptionInfo<'_>`、`ExceptionKind` | 只在本次回调中有效 |

## 许可证

MIT。3DMVS 与 LPSDK 的使用受海康机器人的许可条款约束，本项目不分发 SDK 文件。
