# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK（LPSDK）的安全 Rust 封装。workspace 包含：

- `mv3d-lp`：安全接口；
- `mv3d-lp-sys`：审计过的原始 FFI 与结构体布局断言。

## 支持与安装

- 目标：`x86_64-pc-windows-msvc`
- bindings 基线：LPSDK `1.3.3.3`（已安装的官方开发指南为 V1.3.2，接口表以较新的头文件为准）
- MSRV：Rust `1.87`
- 项目暂不发布到 crates.io，请从 Git 仓库依赖：

```toml
[dependencies]
mv3d-lp = { git = "https://github.com/JSB-Unscarred/3dmvs-sdk-rs.git" }
```

构建脚本通过 `MV3DLP_DEV_ENV` 定位 SDK 的 Development 目录，默认
`C:\Program Files (x86)\3DMVS\Development`。未安装 SDK 时只给出警告，`check`、`clippy` 与 `doc`
仍可运行，链接会失败。`display-windows` feature 增加 Win32 窗口显示。

## 快速开始

```rust,no_run
use std::net::Ipv4Addr;
use std::time::Duration;

use mv3d_lp::Sdk;

fn main() -> mv3d_lp::Result<()> {
    let sdk = Sdk::initialize()?;
    let mut device = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;

    let grabbing = device.start_grabbing()?;
    let image = grabbing.get_image(Some(Duration::from_millis(100)))?;
    println!("{}x{}，{} 字节", image.width, image.height, image.data.len());
    Ok(())
}
```

更多用法见 [`examples/`](examples)：`enumerate_devices`、`grab_pull`、`grab_callback`。

## 生命周期

所有权、类型状态、清理失败策略、callback 与图像约定集中在 rustdoc 的
[`docs::architecture`](src/docs/architecture.rs)。要点：

- `Sdk` 与每个 `Device` 共享 SDK 会话，全部释放后自动 `MV3D_LP_Finalize`；每个进程只能初始化一次。
- `Device::start_grabbing` / `start_grabbing_with` 返回可变借用设备的守卫，pull 取图只存在于
  `Grabbing` 上；守卫释放时停止采集。
- LPSDK 不能注销 callback：闭包保留到 Close，注册过 image callback 的设备不能再用 pull 取图。
- `Drop` 忽略清理错误；需要观察时调用 `Device::close`、`Grabbing::stop` 或 `CallbackGrabbing::stop`。
- 字符串参数使用 `&CStr`，例如 `c"ExposureTime"`。

## SDK 接口对应表

按 LPSDK `1.3.3.3` 的 `Mv3dLpApi.h` 与 `Mv3dLpImgProc.h` 声明顺序排列。所有 `Result` 的错误为
`Error::Sdk { function, code }`，另有 `AlreadyInitialized`、`InvalidInput` 与 `ImageCallbackRegistered`。

| SDK 接口 | 安全 Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version() -> &'static CStr` | 无需初始化 |
| `MV3D_LP_Initialize` | `Sdk::initialize() -> Result<Sdk>` | 每个进程一次 |
| `MV3D_LP_Finalize` | `Sdk` 与全部 `Device` 释放时自动调用 | 某个设备 Close 失败后不再调用 |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count(&self) -> Result<u32>` |  |
| `MV3D_LP_GetDeviceList` | `Sdk::devices(&self) -> Result<Vec<DeviceInfo>>` | 返回条数可能少于先前的计数 |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip(&self, Ipv4Addr) -> Result<Device>` |  |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial(&self, &CStr) -> Result<Device>` | 可直接传入 `DeviceInfo::serial_number()` |
| `MV3D_LP_CloseDevice` | `Device::close(self) -> Result<()>`、`Drop` | 失败时保留闭包与会话引用 |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config(&self, &CStr, IpConfiguration) -> Result<()>` |  |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::register_exception_callback(&mut self, F) -> Result<()>` | `F: Fn(&DeviceException<'_>) + Send + Sync + 'static`；闭包保留到 Close |
| `MV3D_LP_StartMeasure` | `Device::start_grabbing(&mut self) -> Result<Grabbing<'_>>`、`start_grabbing_with` |  |
| `MV3D_LP_StopMeasure` | `Grabbing::stop(self)`、`CallbackGrabbing::stop(self)`、守卫的 `Drop` |  |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger(&self) -> Result<()>` | 触发模式与调用顺序由 SDK 检查 |
| `MV3D_LP_GetImage` | `Grabbing::get_image(&self, Option<Duration>) -> Result<Image>` | `None` 为无限等待；返回前复制 |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_grabbing_with(&mut self, F) -> Result<CallbackGrabbing<'_>>` | `F: Fn(Image) + Send + Sync + 'static`；闭包保留到 Close |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer(&self) -> Result<()>` |  |
| `MV3D_LP_GetParam` | `Device::get_parameter(&self, &CStr) -> Result<Parameter>` |  |
| `MV3D_LP_SetParam` | `Device::set_parameter(&self, &CStr, &ParameterValue) -> Result<()>` | 字符串最多 255 字节 |
| `MV3D_LP_Execute` | `Device::execute(&self, &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessRead` | `Device::download_file(&self, device_file: &CStr, local_file: &CStr) -> Result<()>` |  |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file(&self, local_file: &CStr, device_file: &CStr) -> Result<()>` |  |
| `MV3D_LP_GetFileAccessProgress` | `Device::file_transfer_progress(&self) -> Result<FileProgress>` |  |
| `MV3D_LP_GetDeviceIP` | 未封装 | 废弃；使用 `DeviceInfo::current_ip` |
| `MV3D_LP_GetDeviceSN` | 未封装 | 废弃；使用 `DeviceInfo::serial_number` |
| `MV3D_LP_GetProfile` | 未封装 | 废弃；使用 `Grabbing::get_image` |
| `MV3D_LP_GetBatchProfile` | 未封装 | 废弃；使用 `Grabbing::get_image` |
| `MV3D_LP_GetIntensityData` | 未封装 | 废弃；亮度数据位于 `Image::intensity_data` |
| `MV3D_LP_RegisterProfileCallBack` | 未封装 | 废弃；使用 `Device::start_grabbing_with` |
| `MV3D_LP_RegisterBatchProfileCallBack` | 未封装 | 废弃；使用 `Device::start_grabbing_with` |
| `MV3D_LP_MapDepthToPointCloud` | `Sdk::depth_to_point_cloud(&self, &Image) -> Result<Image>` | 调用前校验输入长度 |
| `MV3D_LP_MapDepthToPointCloudRound` | `Sdk::depth_to_round_point_cloud(&self, &[Image]) -> Result<Image>` | 最多 8 张 |
| `MV3D_LP_ImageConvert` | `Sdk::convert(&self, &Image, ImageType) -> Result<Image>` |  |
| `MV3D_LP_DepthMosaic` | `Sdk::mosaic_depth(&self, &[Image]) -> Result<Image>` | 最多 8 张 |
| `MV3D_LP_SaveImage` | `Sdk::save(&self, &Image, ImageFileFormat, &CStr) -> Result<()>` |  |
| `MV3D_LP_DisplayImage` | `Sdk::display(&self, &Image, &W, DisplayRange) -> Result<()>` | `display-windows` feature；`W: HasWindowHandle` |

## SDK 结构体对应表

| SDK 结构体 | Rust 类型 | 说明 |
| --- | --- | --- |
| `MV3D_LP_DEVICE_INFO` | `DeviceInfo` | 保存原始记录；字符串读取为 `&CStr`，IP 解析为 `Option<Ipv4Addr>` |
| `MV3D_LP_IP_CONFIG` | `IpConfiguration` | Static、DHCP、LinkLocal |
| `MV3D_LP_IMAGE_DATA` | `Image`、`ImageCalibration` | 采集与处理输出复制为拥有值；作为输入时校验长度 |
| `MV3D_LP_PARAM` 及各参数结构体 | `Parameter`、`ParameterValue` | enum 取代 tagged union |
| `MV3D_LP_EXCEPTION_INFO` | `DeviceException<'_>`、`ExceptionKind` | 回调期间借用描述 |
| `MV3D_LP_FILE_ACCESS` | `Device::download_file`、`Device::upload_file` 的参数 | 文件名只在调用期间借出 |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | `FileProgress` | 原样保存计数 |
| `MVB3D_LP_POINT_XYZ_*`、`MV3D_LP_PROFILE_DATA` 等废弃结构体 | `Image` 的字节载荷 | 不公开单独的类型 |

## 开发与验证

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo doc --workspace --no-deps
cargo test --workspace
```

`clippy` 与 `doc` 不需要 SDK；`cargo test` 需要链接并运行 3DMVS DLL。真机测试需要专用设备与
`MV3D_LP_TEST_SERIAL`：

```powershell
cargo test --features hardware-tests --test hardware_smoke -- --ignored
```

## 许可证

本项目采用 [MIT License](LICENSE)。许可证只覆盖本仓库代码，3DMVS/LPSDK 文件与设备的授权仍以厂商条款为准。
