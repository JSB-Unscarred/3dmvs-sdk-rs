# mv3d-lp

海康机器人（Hikrobot）3D 激光轮廓传感器 SDK（LPSDK）的非官方安全 Rust 封装。workspace 包含：

- `mv3d-lp`：安全接口；
- `mv3d-lp-sys`：审计过的原始 FFI 与结构体布局断言，也可经 `mv3d_lp::sys` 访问。

## 支持

- 目标：`x86_64-pc-windows-msvc`，其它目标在编译期报错。
- SDK 基线：LPSDK 1.3.3.3（已安装的官方开发指南为 V1.3.2，接口表以较新的头文件为准）。
- 构建：以 `raw-dylib` 链接 `Mv3dLp.dll`，不需要 SDK 的导入库或环境变量；
  未安装 SDK 时 `check`、`clippy`、`doc` 与单元测试照常运行。
- 运行：安装 3DMVS，`Mv3dLp.dll` 所在目录需要在 `PATH` 中。

## 安装

```toml
[dependencies]
mv3d-lp = "0.1"
```

## 快速开始

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

更多用法见 [`examples/`](examples)：`enumerate_devices`、`grab_pull`、`grab_callback`。

## 生命周期

所有权、类型状态、清理失败策略、callback 与图像约定集中在 rustdoc 的
[`docs::architecture`](https://docs.rs/mv3d-lp/latest/mv3d_lp/docs/architecture/index.html)。要点：

- 本进程只有一个 SDK 会话：会话存活期间 `Sdk::new` 与 `clone` 得到同一会话；`Sdk` 与全部 `Device`
  释放后自动 `MV3D_LP_Finalize`，之后 `Sdk::new` 返回 `Error::Finalized`。Initialize 失败可以重试。
- `Device::start_grabbing` / `start_grabbing_with` 返回可变借用设备的守卫，pull 取图只存在于 `Grabbing` 上；
  守卫释放时停止取流。
- LPSDK 不能注销 callback：闭包由设备保留到 `CloseDevice`，注册过 image callback 的设备不能再用 pull 取图。
  每次回调期间再持有一份闭包引用，在 callback 中释放设备不会释放正在执行的闭包。
- 取流与图像处理的输出都复制为拥有的 `Image`；作为处理输入时只放行能校验长度的格式。
- `Drop` 忽略清理错误；需要观察时调用 `Device::close`、`Grabbing::stop` 或 `CallbackGrabbing::stop`。
- 字符串参数使用 `&CStr`，例如 `c"ExposureTime"`；错误信息为英文。

## SDK 接口对应表

按 LPSDK 1.3.3.3 的 `Mv3dLpApi.h` 与 `Mv3dLpImgProc.h` 声明顺序排列。所有 `Result` 的错误为
`Error::Sdk { function, code }`，另有 `Finalized`、`InvalidInput` 与 `ImageCallbackRegistered`。

| SDK 接口 | 安全 Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version() -> &'static CStr` | 无需初始化 |
| `MV3D_LP_Initialize` | `Sdk::new() -> Result<Sdk>` | 首次调用时初始化；会话存活期间返回同一会话 |
| `MV3D_LP_Finalize` | `Sdk` 与全部 `Device` 释放时自动调用 | 某个设备 `CloseDevice` 失败后不再调用 |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count(&self) -> Result<u32>` |  |
| `MV3D_LP_GetDeviceList` | `Sdk::devices(&self) -> Result<Vec<DeviceInfo>>` | 没有设备时不调用；返回条数可能少于先前的计数 |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip(&self, Ipv4Addr) -> Result<Device>` |  |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial(&self, &CStr) -> Result<Device>` | 可直接传入 `DeviceInfo::serial_number()` |
| `MV3D_LP_CloseDevice` | `Device::close(self) -> Result<()>`、`Drop` | 失败时保留闭包与会话引用 |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config(&self, &CStr, IpConfig) -> Result<()>` |  |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::register_exception_callback(&mut self, F) -> Result<()>` | `F: Fn(ExceptionInfo<'_>) + Send + Sync + 'static`；闭包保留到 `CloseDevice` |
| `MV3D_LP_StartMeasure` | `Device::start_grabbing(&mut self) -> Result<Grabbing<'_>>`、`start_grabbing_with` |  |
| `MV3D_LP_StopMeasure` | `Grabbing::stop(self)`、`CallbackGrabbing::stop(self)`、守卫的 `Drop` |  |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger(&self) -> Result<()>` | 触发模式与调用顺序由 SDK 检查 |
| `MV3D_LP_GetImage` | `Grabbing::get_image(&self, Option<Duration>) -> Result<Image>` | `None` 为无限等待；亚毫秒向上取整；返回前复制 |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_grabbing_with(&mut self, F) -> Result<CallbackGrabbing<'_>>` | `F: Fn(Image) + Send + Sync + 'static`；闭包保留到 `CloseDevice` |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer(&self) -> Result<()>` |  |
| `MV3D_LP_GetParam` | `Device::get_parameter(&self, &CStr) -> Result<Parameter>` | 未知类型为 `Parameter::Other` |
| `MV3D_LP_SetParam` | `Device::set_parameter(&self, &CStr, ParameterValue<'_>) -> Result<()>` | 字符串最多 255 字节 |
| `MV3D_LP_Execute` | `Device::execute_command(&self, &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessRead` | `Device::download_file(&self, device_file: &CStr, local_file: &CStr) -> Result<()>` | 阻塞到传输结束 |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file(&self, local_file: &CStr, device_file: &CStr) -> Result<()>` | 阻塞到传输结束 |
| `MV3D_LP_GetFileAccessProgress` | 未封装 | 传输调用阻塞且 `Device` 不是 `Sync`，传输期间无法查询；需要时经 `mv3d_lp::sys` 调用 |
| `MV3D_LP_GetDeviceIP` | 未封装 | 废弃；使用 `DeviceInfo::current_ip` |
| `MV3D_LP_GetDeviceSN` | 未封装 | 废弃；使用 `DeviceInfo::serial_number` |
| `MV3D_LP_GetProfile` | 未封装 | 废弃；使用 `Grabbing::get_image` |
| `MV3D_LP_GetBatchProfile` | 未封装 | 废弃；使用 `Grabbing::get_image` |
| `MV3D_LP_GetIntensityData` | 未封装 | 废弃；亮度数据位于 `Image::intensity_data` |
| `MV3D_LP_RegisterProfileCallBack` | 未封装 | 废弃；使用 `Device::start_grabbing_with` |
| `MV3D_LP_RegisterBatchProfileCallBack` | 未封装 | 废弃；使用 `Device::start_grabbing_with` |
| `MV3D_LP_MapDepthToPointCloud` | `Sdk::depth_to_point_cloud(&self, &Image) -> Result<Image>` | 调用前校验输入长度 |
| `MV3D_LP_MapDepthToPointCloudRound` | `Sdk::depth_to_round_point_cloud(&self, &[Image]) -> Result<Image>` | 最多 8 张 |
| `MV3D_LP_ImageConvert` | `Sdk::convert_image(&self, &Image, ImageType) -> Result<Image>` |  |
| `MV3D_LP_DepthMosaic` | `Sdk::mosaic_depth(&self, &[Image]) -> Result<Image>` | 最多 8 张 |
| `MV3D_LP_SaveImage` | `Sdk::save_image(&self, &Image, ImageFileFormat, &CStr) -> Result<()>` |  |
| `MV3D_LP_DisplayImage` | `Sdk::display_image(&self, &Image, &W, DisplayRange) -> Result<()>` | `W: HasWindowHandle`，需要 Win32 窗口 |

## SDK 结构体对应表

| SDK 结构体 | Rust 类型 | 说明 |
| --- | --- | --- |
| `MV3D_LP_DEVICE_INFO` | `DeviceInfo` | 保存原始记录；字符串读取为 `&CStr`，IP 解析为 `Option<Ipv4Addr>`，MAC 为 `[u8; 6]`（取前 6 字节） |
| `MV3D_LP_IP_CONFIG` | `IpConfig` | Static、Dhcp、LinkLocal |
| `MV3D_LP_IMAGE_DATA` | `Image`、`ImageCalibration` | 取流与处理输出复制为拥有值；作为输入时只放行能校验长度的格式 |
| `MV3D_LP_PARAM` 及各参数结构体 | `Parameter`、`ParameterValue<'_>` | enum 取代 tagged union，变体对应 `ParamType_*` |
| `MV3D_LP_EXCEPTION_INFO` | `ExceptionInfo<'_>`、`ExceptionKind` | 回调期间借用描述 |
| `MV3D_LP_FILE_ACCESS` | `Device::download_file`、`Device::upload_file` 的参数 | 文件名在阻塞的传输调用期间借出 |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | 未封装 | 见 `MV3D_LP_GetFileAccessProgress` |
| `MVB3D_LP_POINT_XYZ_*`、`MV3D_LP_PROFILE_DATA` 等废弃结构体 | `Image` 的字节载荷 | 不公开单独的类型 |

## 开发与验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo doc --workspace --no-deps
cargo test --workspace
```

`cargo test --workspace` 会启动真机测试程序，需要 `Mv3dLp.dll` 在 `PATH` 中；未安装 SDK 时改用
`cargo test --workspace --lib --test thread_traits` 与 `cargo test --workspace --doc`。真机测试只操作专用设备：

```powershell
$env:MV3D_LP_TEST_SERIAL = "<设备序列号>"
cargo test --test hardware_smoke -- --ignored
```

## 许可证

本项目采用 [MIT License](LICENSE)。许可证只覆盖本仓库代码，3DMVS/LPSDK 文件与设备的授权仍以厂商条款为准。
