# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK 的 safe Rust 包装。公共 crate 只提供 Rust 类型、生命周期和错误模型，原始句柄、裸指针、C union 与 FFI 集中在私有 crate。

## 支持与安装

- 原生目标：`x86_64-pc-windows-msvc`
- bindings 基线：LPSDK `1.3.3.3`
- 默认兼容范围：`>=1.3.3.3, <1.3.4.0`
- MSRV：Rust `1.85`
- 默认 feature 为空；`native` 启用 SDK，`display-windows` 增加 Win32 图像显示
- 两个 crate 均设置 `publish = false`，本项目暂不发布至 crates.io

请从 Git 仓库依赖：

```toml
[dependencies]
mv3d-lp = { git = "https://github.com/JSB-Unscarred/3dmvs-sdk-rs.git", features = ["native"] }
```

`native` 要求安装 3DMVS。构建脚本通过 `MV3DLP_DEV_ENV` 定位 Development 目录，默认值为：

```text
C:\Program Files (x86)\3DMVS\Development
```

默认构建只提供类型与 API，`Sdk::initialize()` 返回 `Error::UnsupportedPlatform`。

## 快速开始

```rust,no_run
use std::{net::Ipv4Addr, time::Duration};

use mv3d_lp::{Result, Sdk};

fn main() -> Result<()> {
    let sdk = Sdk::initialize()?;

    for info in sdk.devices()? {
        println!("{}", info.model_name.to_string_lossy());
    }

    let mut device = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;
    let mut measurement = device.start()?;
    let frame = measurement.get_image(Duration::from_millis(100))?;
    println!("{}x{}, {} bytes", frame.width, frame.height, frame.data.len());

    measurement.stop()?;
    device.close()?;
    sdk.shutdown()
}
```

设备也可通过 `SerialNumber` 与 `Sdk::open_by_serial()` 打开。SDK 文本使用 `SdkText` 保存原始有界字节，可按需调用 `to_str()` 或 `to_string_lossy()`。

## SDK 接口对应表

下表以项目审计的 LPSDK `1.3.3.3` 头文件为准。`Drop` 只做尽力清理；需要观察清理错误时应调用显式的 `stop()`、`close()` 和 `shutdown()`。

| SDK 接口 | safe Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version()` | 初始化时读取并解析版本 |
| `MV3D_LP_Initialize` | `Sdk::initialize()`、`Sdk::initialize_strict()` | 管理进程级 SDK 生命周期 |
| `MV3D_LP_Finalize` | `Sdk::shutdown()`、`Sdk` 的 `Drop` | 显式关闭可返回错误 |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count_hint()` | 返回枚举容量提示 |
| `MV3D_LP_GetDeviceList` | `Sdk::devices()` | 返回拥有化的 `Vec<DeviceInfo>` |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip()` | `Device<'sdk>` 绑定 SDK 生命周期 |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial()` | 使用校验后的 `SerialNumber` |
| `MV3D_LP_CloseDevice` | `Device::close()`、`Device` 的 `Drop` | 关闭后句柄不可再用 |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config()` | 使用 `IpConfiguration` 表达配置模式 |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::exception_receiver()`、`Device::on_exception()` | 回调数据先复制，再交给 Rust channel 或 worker |
| `MV3D_LP_StartMeasure` | `Device::start()`、`Device::start_receiving()`、`Device::start_with_callback()` | 返回独占借用设备的采集 guard |
| `MV3D_LP_StopMeasure` | `Measurement::stop()`、`CallbackMeasurement::stop()` 及各自 `Drop` | callback 停止前先撤销并排空 Rust 回调 |
| `MV3D_LP_SoftTrigger` | `Measurement::soft_trigger()`、`CallbackMeasurement::soft_trigger()` | 仅在采集中可用 |
| `MV3D_LP_GetImage` | `Measurement::get_image()` | 校验并复制为 `OwnedFrame` |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_receiving()`、`Device::start_with_callback()` | 使用有界、非阻塞 callback queue |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer()`、`Measurement::clear_buffer()` | callback 采集 guard 不公开该操作 |
| `MV3D_LP_GetParam` | `Device::get_parameter()`、`Measurement::get_parameter()` | 返回 `Parameter` |
| `MV3D_LP_SetParam` | `Device::set_parameter()`、`Measurement::set_parameter()` | 接收 `ParameterValue` |
| `MV3D_LP_Execute` | `Device::execute()`、`Measurement::execute()` | key 使用 `CommandKey` 校验 |
| `MV3D_LP_FileAccessRead` | `Device::download_file()` | 返回 `FileTransfer` guard |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file()` | 返回 `FileTransfer` guard |
| `MV3D_LP_GetFileAccessProgress` | `FileTransfer::progress()`、`FileTransfer::wait_timeout()` | 返回校验后的进度快照 |
| `MV3D_LP_GetDeviceIP`、`MV3D_LP_GetDeviceSN` | `Sdk::devices()` | 统一从 `DeviceInfo` 读取 IP 与序列号 |
| `MV3D_LP_GetProfile`、`MV3D_LP_GetBatchProfile`、`MV3D_LP_GetIntensityData` | `Measurement::get_image()` | 暂不直接封装，采集统一使用 `OwnedFrame` |
| `MV3D_LP_RegisterProfileCallBack`、`MV3D_LP_RegisterBatchProfileCallBack` | `Device::start_receiving()`、`Device::start_with_callback()` | 暂不直接封装，回调统一使用 `OwnedFrame` |
| `MV3D_LP_MapDepthToPointCloud` | `ImageProcessor::depth_to_point_cloud()` | 返回 `OwnedImage` |
| `MV3D_LP_MapDepthToPointCloudRound` | `ImageProcessor::depth_to_round_point_cloud()` | 输入数量限制为 1 至 8 |
| `MV3D_LP_ImageConvert` | `ImageProcessor::convert()` | 调用前校验转换组合与图像布局 |
| `MV3D_LP_DepthMosaic` | `ImageProcessor::mosaic_depth()` | 输入数量限制为 1 至 8 |
| `MV3D_LP_SaveImage` | `ImageProcessor::save()` | 使用 `ImageFileFormat` 限定格式 |
| `MV3D_LP_DisplayImage` | `ImageProcessor::display()` | 仅 `display-windows` feature 提供 |

## SDK 结构体对应表

| SDK 结构体 | Rust 定义 | 说明 |
| --- | --- | --- |
| `MV3D_LP_DEVICE_INFO` | `DeviceInfo` | 字符串、MAC 与 IPv4 字段转为拥有值 |
| `MV3D_LP_IP_CONFIG` | `IpConfiguration` | 用 enum 表达 Static、DHCP 与 LinkLocal |
| `MV3D_LP_IMAGE_DATA` | `ImageRef<'_>`、`OwnedFrame`、`OwnedImage` | 输入使用借用，SDK 输出立即复制 |
| `MV3D_LP_INTPARAM` | `Parameter::Integer`、`ParameterValue::Integer` | 保留当前值、范围与步长 |
| `MV3D_LP_ENUMPARAM` | `Parameter::Enumeration`、`ParameterValue::Enumeration` | 支持值复制为 `Vec<u32>` |
| `MV3D_LP_FLOATPARAM` | `Parameter::Float`、`ParameterValue::Float` | 保留当前值与范围 |
| `MV3D_LP_STRINGPARAM` | `Parameter::String`、`ParameterValue::String` | 内容使用 `SdkText` 保存 |
| `MV3D_LP_PARAM_INFO`、`MV3D_LP_PARAM` | `Parameter`、`ParameterValue` | 通过 enum 取代 C union 与判别字段 |
| `MV3D_LP_EXCEPTION_INFO` | `DeviceException`、`DeviceExceptionType` | 拥有化描述并保留未知类型值 |
| `MV3D_LP_FILE_ACCESS` | `Device::download_file()`、`Device::upload_file()`、`FileTransfer<'_>` | 文件名借用由内部层转换并保活 |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | `FileProgress`、`FileTransferStatus` | 使用非负计数表示运行或完成 |
| `MVB3D_LP_POINT_XYZ_S16`、`MVB3D_LP_POINT_XYZ_F32` | `OwnedFrame`、`OwnedImage` 的字节载荷 | 暂不公开单点结构体 |
| `MV3D_LP_PROFILE_DATA`、`MV3D_LP_DEPTH_DATA`、`MV3D_LP_INTENSITY_DATA` | `OwnedFrame` | 暂不直接映射旧采集结构体 |
| `MV3D_LP_POINTCLOUD_DATA` | `OwnedImage` | 点云统一表示为带 `ImageType` 的拥有化图像 |

SDK 的 reserved 字段、原始指针、回调函数指针和设备句柄只存在于私有 crate。

## 安全边界

- 公共 crate 使用 `#![forbid(unsafe_code)]`；FFI、指针校验、C union 读取和 callback trampoline 位于 `mv3d-lp-internal`。
- SDK 输出先校验判别值、指针、长度和算术，再复制到 Rust 所有值。
- `Device` 借用 `Sdk`；采集与文件传输 guard 独占借用 `Device`，用生命周期限制调用顺序。
- `Sdk` 与 `ImageProcessor` 为 `!Send + !Sync`；`Device` 及其 guard 为 `Send + !Sync`，只允许转移唯一所有权。
- callback cookie 永不复用，用户 handler 在独立 Rust worker 上运行；队列满时丢弃最新事件。
- 清理结果不确定时进程生命周期进入 `Degraded`，后续初始化、新设备 open 与 `Finalize` 会被拒绝。

safe API 依赖同步复制期间输入和 SDK 输出保持有效，以及 Stop/Close 对资源的厂商契约。SDK、头文件、ABI 或固件变化后应重新审计相关接口。

## 开发与验证

默认验证不连接设备：

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked
cargo doc --workspace --no-deps --locked
```

原生环境可追加：

```powershell
cargo check --workspace --features native --locked
cargo test --workspace --features native --no-run --locked
cargo test --workspace --features display-windows --locked
```

## 许可证

本项目采用 [MIT License](LICENSE)。许可证只覆盖本仓库代码，3DMVS/LPSDK 文件与设备的授权仍以厂商条款为准。
