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
    device.start()?;
    let frame = device.get_image(Duration::from_millis(100))?;
    println!("{}x{}, {} bytes", frame.width, frame.height, frame.data.len());

    device.stop()?;
    device.close()?;
    sdk.shutdown()
}
```

设备也可通过 `SerialNumber` 与 `Sdk::open_by_serial()` 打开。SDK 文本使用 `SdkText` 保存原始有界字节，可按需调用 `to_str()` 或 `to_string_lossy()`。

## 状态 API 迁移

采集状态现由 `Device` 自身持有，`Measurement` 与 `CallbackMeasurement` 已移除：

| 旧调用 | 当前调用 |
| --- | --- |
| `let mut measurement = device.start()?` | `device.start()?` |
| `measurement.get_image(timeout)?` | `device.get_image(timeout)?` |
| `measurement.soft_trigger()?` | `device.soft_trigger()?` |
| `measurement.stop()?` | `device.stop()?` |
| `let (measurement, receiver) = device.start_receiving(options)?` | `let receiver = device.start_receiving(options)?` |
| `let (measurement, worker) = device.start_with_callback(options, handler)?` | `let worker = device.start_with_callback(options, handler)?` |

`Receiver<OwnedFrame>`、`CallbackWorker` 与 `OwnedFrame` 都不借用 `Device`。callback 仍由 `device.stop()` 显式结束；需要等待 worker 时，先 stop，再调用 `worker.join()`。

文件传输也由 `Device` 持有状态和文件名：

| 旧调用 | 当前调用 |
| --- | --- |
| `let mut transfer = device.download_file(device_name, local_name)?` | `device.download_file(device_name, local_name)?` |
| `let mut transfer = device.upload_file(local_name, device_name)?` | `device.upload_file(local_name, device_name)?` |
| `transfer.progress()?` | `device.file_transfer_progress()?` |
| `transfer.wait_timeout(interval, timeout)?` | `device.wait_file_transfer(interval, timeout)?` |
| `drop(transfer); device.active_file_transfer()` | 直接继续使用 `device` 轮询 |
| `FileTransferDirection` | 由 `download_file()` 与 `upload_file()` 的方法名表达方向 |

`download_file()` 与 `upload_file()` 只启动传输。设备随后处于 `Transferring`；观察到完成时恢复 `Open`。轮询错误或本地等待超时保留传输状态，可以继续调用进度接口。活动传输需要跨线程时，直接移动 `Device`。

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
| `MV3D_LP_RegisterExceptionCallBack` | `Device::exception_receiver()`、`Device::on_exception()`、`Device::disable_exception_delivery()` | 事件先复制；disable 撤销并排空 Rust delivery |
| `MV3D_LP_StartMeasure` | `Device::start()`、`Device::start_receiving()`、`Device::start_with_callback()` | `Device` 持有采集状态，start 只短暂借用 `&mut self` |
| `MV3D_LP_StopMeasure` | `Device::stop()`、`Device::close()` 及 `Drop` 兜底 | callback 停止前先撤销并排空 Rust 回调 |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger()` | 仅在采集中可用 |
| `MV3D_LP_GetImage` | `Device::get_image()` | 仅用于 pull 采集，校验并复制为 `OwnedFrame` |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_receiving()`、`Device::start_with_callback()` | 使用有界、非阻塞 callback queue |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer()` | Open 或 pull 采集状态可用 |
| `MV3D_LP_GetParam` | `Device::get_parameter()` | 返回 `Parameter` |
| `MV3D_LP_SetParam` | `Device::set_parameter()` | 接收 `ParameterValue` |
| `MV3D_LP_Execute` | `Device::execute()` | key 使用 `CommandKey` 校验 |
| `MV3D_LP_FileAccessRead` | `Device::download_file()` | 启动下载，`Device` 保持 `Transferring` |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file()` | 启动上传，`Device` 保持 `Transferring` |
| `MV3D_LP_GetFileAccessProgress` | `Device::file_transfer_progress()`、`Device::wait_file_transfer()` | 返回校验后的进度快照；观察到完成后恢复 `Open` |
| `MV3D_LP_GetDeviceIP`、`MV3D_LP_GetDeviceSN` | `Sdk::devices()` | 统一从 `DeviceInfo` 读取 IP 与序列号 |
| `MV3D_LP_GetProfile`、`MV3D_LP_GetBatchProfile`、`MV3D_LP_GetIntensityData` | `Device::get_image()` | 暂不直接封装，采集统一使用 `OwnedFrame` |
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
| `MV3D_LP_FILE_ACCESS` | `Device::download_file()`、`Device::upload_file()` | `Device` 在原生异步传输期间保活文件名 |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | `FileProgress`、`FileTransferStatus` | 使用非负计数表示运行或完成 |
| `MVB3D_LP_POINT_XYZ_S16`、`MVB3D_LP_POINT_XYZ_F32` | `OwnedFrame`、`OwnedImage` 的字节载荷 | 暂不公开单点结构体 |
| `MV3D_LP_PROFILE_DATA`、`MV3D_LP_DEPTH_DATA`、`MV3D_LP_INTENSITY_DATA` | `OwnedFrame` | 暂不直接映射旧采集结构体 |
| `MV3D_LP_POINTCLOUD_DATA` | `OwnedImage` | 点云统一表示为带 `ImageType` 的拥有化图像 |

SDK 的 reserved 字段、原始指针、回调函数指针和设备句柄只存在于私有 crate。

## 安全边界

- 公共 crate 使用 `#![forbid(unsafe_code)]`；FFI、指针校验、C union 读取和 callback trampoline 位于 `mv3d-lp-internal`。
- SDK 输出先校验判别值、指针、长度和算术，再复制到 Rust 所有值。
- `Device` 借用 `Sdk`，防止设备仍打开时提前 Finalize；采集、文件传输状态和异步文件名都由 `Device` 持有。Close 失败时原生传输的终止状态不确定，文件名存储会被故意保留以防悬空指针。
- `Sdk` 与 `ImageProcessor` 为 `!Send + !Sync`；`Device` 为 `Send + !Sync`，活动采集或传输可随设备的唯一所有权跨线程移动。
- callback registration 和永不复用的 cookie 由 `Device` 持有；`stop()` 先撤销准入并排空 in-flight callback，再调用原生 Stop。用户 handler 在独立 Rust worker 上运行，队列满时丢弃最新事件。
- 厂商 exception callback 接口只提供 register。`disable_exception_delivery()` 在本地撤销 cookie 并排空 in-flight callback；原生晚到调用仍可能发生，由 registry 忽略已撤销 cookie。
- Stop 失败会把设备置为 `Faulted`；除本地撤销 exception delivery 外，此后只允许 `close()` 或 `Drop` 兜底重试 Stop 并尝试关闭句柄，晚到 callback 按已撤销 cookie 隔离。
- Close/Finalize 失败、仍有 live handle 时请求 shutdown 或 handle ledger 不确定会使进程生命周期进入 `Degraded`，后续初始化、新设备 open 与 `Finalize` 会被拒绝。

safe API 依赖同步复制期间输入和 SDK 输出保持有效，以及 Stop/Close 对资源的厂商契约。SDK、头文件、ABI 或固件变化后应重新审计相关接口。

## 生命周期文档

- [生命周期与时序图总览](生命周期与时序图.md)
- [标准生命周期与 pull 采集](时序图/标准生命周期与-pull-采集.md)
- [callback 采集与停止](时序图/callback-采集与停止.md)
- [文件上传与下载](时序图/文件上传与下载.md)

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
