# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK 的 safe Rust 包装。公共 crate 只提供 Rust 类型、所有权和错误模型，原始句柄、裸指针、C union 与 FFI 集中在私有 crate。

## 支持与安装

- 原生目标：`x86_64-pc-windows-msvc`
- bindings 基线：LPSDK `1.3.3.3`
- 兼容范围：`[1.3.3.3, 1.3.4.0)`
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

需要无限等待下一帧时调用 `device.get_image_blocking()`；`get_image(Duration)` 只表达有限等待。

## 所有权与状态

- `Sdk` 是 `Send + Sync` 的 session token。进程状态只有 `Fresh` 与 `Active`：`initialize()` 创建或加入活动 session，Initialize 失败时仍为 `Fresh`；`shutdown()` 显式 Finalize，Finalize 失败时仍为 `Active`，两种失败均可重试。
- `Device` 不借用 `Sdk`，可在释放 `Sdk` 后继续使用，也可移动到普通 worker thread。它是 `Send + !Sync`；live device 会让 `shutdown()` 返回可重试错误。原生 Close 成功才减少 live handle 计数；显式 `close(self)` 首次失败时由同一 owner 的 Drop 再试一次，连续失败或直接 Drop 失败才继续阻止 Finalize。
- `ImageProcessor` 不借用 `Sdk`，是 `Send + Sync` 的图像处理 token。
- `shutdown()` 成功后，同一 session 的其他 token 停止接受 native 操作；缓存的 `Sdk::version()` 仍可读取。

采集与文件传输状态由 `Device` 持有。`start()`、`stop()` 和文件传输方法只短暂借用设备；`Receiver<Frame>` 与 `Receiver<DeviceException>` 均不借用 `Device`。Receiver 可在当前线程消费，也可由调用方移入自建线程。结束 image callback 时调用 `device.stop()`，结束 exception delivery 时调用 `disable_exception_delivery()`；随后等待 channel 断开并结束自建线程。

`Frame` 是 `Image` 的类型别名；采集和图像处理共用同一拥有类型。它拥有主数据及可选的亮度数据、曝光时间戳，可脱离 SDK 缓冲区使用；`from_image_ref()` 与 `clone()` 深拷贝这三类 payload，`as_image_ref()` 返回借用视图。因包含 `Vec`，该类型为 `Clone + !Copy`。

## SDK 接口对应表

下表以项目审计的 LPSDK `1.3.3.3` 头文件为准。`Device` 的 `Drop` 只做尽力清理；需要观察首次清理错误时应显式调用 `stop()` 与 `close()`。显式 Close 失败后的 Drop 重试结果不单独返回。Finalize 及其错误只由 `shutdown()` 处理。

| SDK 接口 | safe Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version()` | 初始化时读取并解析版本 |
| `MV3D_LP_Initialize` | `Sdk::initialize()` | 固定校验 `[1.3.3.3, 1.3.4.0)`；成功时 `Fresh -> Active`，失败后仍可重试 |
| `MV3D_LP_Finalize` | `Sdk::shutdown()` | 成功时 `Active -> Fresh`；失败时仍为 `Active`，可重试 |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count_hint()` | 返回枚举容量提示 |
| `MV3D_LP_GetDeviceList` | `Sdk::devices()` | 按一次 `GetDeviceNumber` 的结果调用一次；计数为 0 时直接返回空列表 |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip()` | status 成功且 handle 非空后才创建 `Device` |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial()` | 校验 `SerialNumber`；成功后才交付 handle |
| `MV3D_LP_CloseDevice` | `Device::close()`、`Device` 的 `Drop` | 成功才释放 handle 并减少计数；显式 Close 首次失败后由 Drop 再试一次，直接 Drop 只尽力调用一次 |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config()` | 使用 `IpConfiguration` 表达配置模式 |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::exception_receiver()`、`Device::disable_exception_delivery()` | 返回有界 `Receiver<DeviceException>`；registry 撤销与 drain 是待厂商时序确认的保守措施 |
| `MV3D_LP_StartMeasure` | `Device::start()`、`Device::start_receiving()` | `Device` 持有采集状态，start 只短暂借用 `&mut self` |
| `MV3D_LP_StopMeasure` | `Device::stop()`、`Device::close()` 及 `Drop` 兜底 | callback 路径当前先撤销并排空 Rust 回调 |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger()` | 仅在采集中可用 |
| `MV3D_LP_GetImage` | `Device::get_image()`、`Device::get_image_blocking()` | pull 采集的有限等待与无限等待；internal FFI 校验后复制为 `Frame`（即 `Image`） |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_receiving()` | 返回有界 `Receiver<Frame>`；callback 入队不阻塞 |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer()` | 直接转发；允许状态待厂商确认 |
| `MV3D_LP_GetParam` | `Device::get_parameter()` | 接收 `ParamKey`，返回 `Parameter` |
| `MV3D_LP_SetParam` | `Device::set_parameter()` | 接收 `ParamKey` 与 `ParameterValue` |
| `MV3D_LP_Execute` | `Device::execute()` | 接收 `CommandKey` |
| `MV3D_LP_FileAccessRead` | `Device::download_file()` | `[IN]` 调用期借用；成功进入 `Transferring`，失败仍为 `Open` |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file()` | `[IN]` 调用期借用；成功进入 `Transferring`，失败仍为 `Open` |
| `MV3D_LP_GetFileAccessProgress` | `Device::file_transfer_progress()` | 返回校验后的单次进度快照；轮询、backoff 或 async 调度由调用方负责 |
| `MV3D_LP_GetDeviceIP`、`MV3D_LP_GetDeviceSN` | `Sdk::devices()` | 统一从 `DeviceInfo` 读取 IP 与序列号 |
| `MV3D_LP_GetProfile`、`MV3D_LP_GetBatchProfile`、`MV3D_LP_GetIntensityData` | `Device::get_image()`、`Device::get_image_blocking()` | 暂不直接封装，采集统一使用 `Frame` |
| `MV3D_LP_RegisterProfileCallBack`、`MV3D_LP_RegisterBatchProfileCallBack` | `Device::start_receiving()` | 暂不直接封装，回调统一使用 `Receiver<Frame>` |
| `MV3D_LP_MapDepthToPointCloud` | `ImageProcessor::depth_to_point_cloud()` | internal FFI 校验输入与输出，返回 `Image` |
| `MV3D_LP_MapDepthToPointCloudRound` | `ImageProcessor::depth_to_round_point_cloud()` | 输入数量限制为 1 至 8 |
| `MV3D_LP_ImageConvert` | `ImageProcessor::convert()` | 图像 descriptor 与布局校验集中在 internal FFI |
| `MV3D_LP_DepthMosaic` | `ImageProcessor::mosaic_depth()` | 输入数量限制为 1 至 8 |
| `MV3D_LP_SaveImage` | `ImageProcessor::save()` | 使用 `ImageFileFormat` 限定格式 |
| `MV3D_LP_DisplayImage` | `ImageProcessor::display()` | 仅 `display-windows` feature 提供 |

## SDK 结构体对应表

| SDK 结构体 | Rust 定义 | 说明 |
| --- | --- | --- |
| `MV3D_LP_DEVICE_INFO` | `DeviceInfo` | 字符串、MAC 与 IPv4 字段转为拥有值 |
| `MV3D_LP_IP_CONFIG` | `IpConfiguration` | 用 enum 表达 Static、DHCP 与 LinkLocal |
| `MV3D_LP_IMAGE_DATA` | `ImageRef<'_>`、`Image`（`Frame` 为别名） | 输入使用借用，采集与处理输出共用拥有类型 |
| `MV3D_LP_INTPARAM` | `Parameter::Integer`、`ParameterValue::Integer` | 保留当前值、范围与步长 |
| `MV3D_LP_ENUMPARAM` | `Parameter::Enumeration`、`ParameterValue::Enumeration` | 支持值复制为 `Vec<u32>` |
| `MV3D_LP_FLOATPARAM` | `Parameter::Float`、`ParameterValue::Float` | 保留当前值与范围 |
| `MV3D_LP_STRINGPARAM` | `Parameter::String`、`ParameterValue::String` | 内容使用 `SdkText` 保存 |
| `MV3D_LP_PARAM_INFO`、`MV3D_LP_PARAM` | `Parameter`、`ParameterValue` | 通过 enum 取代 C union 与判别字段 |
| `MV3D_LP_EXCEPTION_INFO` | `DeviceException`、`DeviceExceptionType` | 拥有化描述并保留未知类型值 |
| `MV3D_LP_FILE_ACCESS` | `Device::download_file()`、`Device::upload_file()` | descriptor 与两段字符串只借用至原生调用返回 |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | `FileProgress`、`FileTransferStatus` | 使用非负计数表示运行或完成 |
| `MVB3D_LP_POINT_XYZ_S16`、`MVB3D_LP_POINT_XYZ_F32` | `Image`（`Frame` 为别名）的字节载荷 | 暂不公开单点结构体 |
| `MV3D_LP_PROFILE_DATA`、`MV3D_LP_DEPTH_DATA`、`MV3D_LP_INTENSITY_DATA` | `Frame` | 暂不直接映射旧采集结构体 |
| `MV3D_LP_POINTCLOUD_DATA` | `Image` | 点云统一表示为带 `ImageType` 的拥有化图像 |

SDK 的 reserved 字段、原始指针、回调函数指针和设备句柄只存在于私有 crate。

## 安全边界

- 公共 crate 使用 `#![forbid(unsafe_code)]`；FFI、指针校验、C union 读取和 callback trampoline 位于 `mv3d-lp-internal`。
- 原生图像输入与输出的判别值、指针、长度、布局和算术校验集中在 internal FFI，再复制到 Rust 所有值；校验依据实际 slice 和 SDK 长度字段，不设置任意的 512 MiB 上限。
- `Device` 独立持有 session 使用权；live handle 会让 `shutdown()` 返回可重试错误，防止设备仍打开时 Finalize。原生 Close 成功后才释放 handle 并减少 `live_handles`。显式 `close(self)` 首次失败时 handle 留在同一 owner 中供 Drop 重试；连续失败，或直接 Drop 的一次 Close 失败，都会让 Finalize 返回 `UnclosedDevices`，但不限制后续 open。
- FileAccess descriptor 与字符串依据头文件 `[IN]` 标记按调用期借用；若厂商另有异步借用要求，需要重新审计该边界。
- `Sdk` 与 `ImageProcessor` 为 `Send + Sync`；`Device` 为 `Send + !Sync`，活动采集或传输可随设备的唯一所有权跨线程移动。
- callback registration 和不复用的 cookie 由 `Device` 持有；当前 wrapper 先撤销准入并排空 in-flight callback，再调用原生 Stop。callback 只向有界 Receiver 入队，消费线程由调用方选择；队列满时丢弃最新事件。
- 厂商 exception callback 接口只提供 register。`disable_exception_delivery()` 在本地撤销 cookie 并 drain；registry 对旧 cookie 的处理属于保守兼容策略。
- Stop 失败会把设备置为 `Faulted`；后续采集控制收敛到 `close()` 或 `Drop`，清理时重试 Stop 并尝试关闭句柄。清理中的 Stop 成功后先回到 `Open`，因此 Close 失败后的 Drop 不重复 Stop。参数、Execute 与 ClearDataBuffer 仍直接转发，其状态契约待确认。
- 仍有 live handle 时 shutdown 返回 `UnclosedDevices`，关闭设备后可直接重试。Initialize 失败时进程仍为 `Fresh`；Finalize 失败时仍为 `Active`，session 操作与后续 shutdown 重试继续可用。

safe API 依赖同步调用期间输入可读、SDK 输出在复制完成前有效，以及 Stop/Close 对资源的厂商契约。SDK、头文件、ABI 或固件变化后应重新审计相关接口。

## 待确认厂商契约

- `MV3D_LP_StopMeasure` 与 `MV3D_LP_CloseDevice` 返回时，image/exception callback 是否已静默。
- 同一 handle 是否支持重复注册 callback，以及跨 Stop、pull/callback 模式切换时旧 registration 的替换规则。
- FileAccess 进度 `(completed, total) == (0, 0)` 表示未开始、进行中、空文件还是完成。
- Get/Set/Execute/ClearDataBuffer 在各设备状态下的允许矩阵，以及 `ParamKey`/`CommandKey` 的编码、字符集和最大长度。

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
