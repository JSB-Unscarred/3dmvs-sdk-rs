# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK 的 safe Rust 包装。公共 crate 只提供 Rust 类型、所有权和错误模型，原始句柄、裸指针、C union 与 FFI 集中在私有 crate。

## 支持与安装

- 原生目标：`x86_64-pc-windows-msvc`
- bindings 基线：LPSDK `1.3.3.3`
- 已安装的官方开发指南为 V1.3.2；接口表以较新的 `1.3.3.3` 头文件为准
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

默认构建只提供类型与 API；`Sdk::version()`、`Sdk::initialize()` 等原生入口返回 `Error::UnsupportedPlatform`。

## 快速开始

```rust,no_run
use std::{net::Ipv4Addr, time::Duration};

use mv3d_lp::{Result, Sdk};

fn main() -> Result<()> {
    println!("SDK: {}", Sdk::version()?);
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

设备也可通过 `SerialNumber` 与 `Sdk::open_by_serial()` 打开。SDK 文本使用 `SdkText` 保存原始字节，可按需调用 `to_str()` 或 `to_string_lossy()`。

需要无限等待下一帧时调用 `device.get_image_blocking()`；`get_image(Duration)` 只表达有限等待。

## 所有权与状态

- `Sdk` 是 `Send + Sync` 的 session token。`Sdk::version()` 可在 Initialize 前独立读取原始版本字节；`initialize()` 只负责创建或加入进程级 session，不设置额外兼容区间。
- `Device` 不借用 `Sdk`，可在释放 `Sdk` 后继续使用，也可移动到普通 worker thread。它是 `Send + !Sync`；live device 会阻止 `shutdown()`。每个 `Device` owner 最多调用一次原生 Close，显式 `close(self)` 返回 Stop 与 Close 的清理错误，`Drop` 只执行同一条尽力清理路径。
- `ImageProcessor` 不借用 `Sdk`，是 `Send + Sync` 的图像处理 token。
- `shutdown()` 成功后，同一 session 的其他 token 停止接受 native 操作；静态 `Sdk::version()` 不依赖 session 状态。

`Device` 只记录是否正在测量，以及 image/exception callback registration 和最近一次成功 FileAccess 的两段文件名。`start()` 与 `stop()` 维护测量标记；其余设备操作直接转发，由 SDK 判断调用顺序。`Receiver<Frame>` 与 `Receiver<DeviceException>` 均不借用 `Device`，可在当前线程消费，也可移入调用方线程。结束 image callback 时调用 `device.stop()`；结束 exception delivery 时调用 `disable_exception_delivery()`。

`Frame` 是 `Image` 的类型别名；采集和图像处理共用同一拥有类型。它拥有主数据及可选的亮度数据、曝光时间戳，可脱离 SDK 缓冲区使用；`from_image_ref()` 与 `clone()` 深拷贝这三类 payload，`as_image_ref()` 返回借用视图。因包含 `Vec`，该类型为 `Clone + !Copy`。

## SDK 接口对应表

下表严格依照 LPSDK `1.3.3.3` 的 `Mv3dLpApi.h` 与 `Mv3dLpImgProc.h` 声明顺序。已安装的 CHM 为 V1.3.2，缺少 `ClearDataBuffer` 和 FileAccess 接口，callback 的目录顺序也与较新头文件不同。`Device` 的 `Drop` 只做尽力清理；需要观察 Stop 与 Close 错误时应显式调用 `close()`。Finalize 及其错误只由 `shutdown()` 处理。

| SDK 接口 | safe Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version()` | 不依赖 Initialize，返回 `SdkText` 原始字节；不解析版本段数 |
| `MV3D_LP_Initialize` | `Sdk::initialize()` | 创建或加入进程级 session，不额外限制版本区间 |
| `MV3D_LP_Finalize` | `Sdk::shutdown()` | live `Device` 为零时调用并返回原生 status |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count_hint()` | 返回枚举容量提示 |
| `MV3D_LP_GetDeviceList` | `Sdk::devices()` | 按一次 `GetDeviceNumber` 的结果调用一次；计数为 0 时直接返回空列表 |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip()` | status 成功且 handle 非空后才创建 `Device` |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial()` | 校验 `SerialNumber`；成功后才交付 handle |
| `MV3D_LP_CloseDevice` | `Device::close()`、`Device` 的 `Drop` | 每个 owner 最多调用一次；测量中先尝试一次 Stop，再调用一次 Close |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config()` | 使用 `IpConfiguration` 表达配置模式 |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::exception_receiver()`、`Device::disable_exception_delivery()` | 返回有界 `Receiver<DeviceException>`；cookie registry 隔离迟到 callback，不等待已开始的 callback |
| `MV3D_LP_StartMeasure` | `Device::start()`、`Device::start_receiving()` | 成功后记录正在测量；callback 入口先注册再 Start |
| `MV3D_LP_StopMeasure` | `Device::stop()`、`Device::close()` 及 `Drop` 兜底 | `stop()` 成功后撤销 image cookie；失败时可再次调用 `stop()` 或关闭 |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger()` | 直接转发，由 SDK 判断调用顺序 |
| `MV3D_LP_GetImage` | `Device::get_image()`、`Device::get_image_blocking()` | pull 采集的有限等待与无限等待；internal FFI 校验后复制为 `Frame`（即 `Image`） |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_receiving()` | 返回有界 `Receiver<Frame>`；callback 入队不阻塞 |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer()` | 直接转发；允许状态待厂商确认 |
| `MV3D_LP_GetParam` | `Device::get_parameter()` | 接收 `&str` Node Name，返回 `Parameter` |
| `MV3D_LP_SetParam` | `Device::set_parameter()` | 接收 `&str` Node Name 与 `ParameterValue` |
| `MV3D_LP_Execute` | `Device::execute()` | 接收 `&str` Command Node Name |
| `MV3D_LP_FileAccessRead` | `Device::download_file()` | 成功后由 `Device` 保留两段 CString，直至下一次成功传输或关闭 |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file()` | 成功后由 `Device` 保留两段 CString，直至下一次成功传输或关闭 |
| `MV3D_LP_GetFileAccessProgress` | `Device::file_transfer_progress()` | 返回 `i64` 原始进度快照，不解释完成状态 |
| `MV3D_LP_GetDeviceIP` | 未直接封装 | 废弃接口；功能替代为从 `Sdk::devices()` 返回的 `DeviceInfo` 读取 IP |
| `MV3D_LP_GetDeviceSN` | 未直接封装 | 废弃接口；功能替代为从 `Sdk::devices()` 返回的 `DeviceInfo` 读取序列号 |
| `MV3D_LP_GetProfile` | 未直接封装 | 废弃接口；功能替代为 `Device::get_image()` 或 `get_image_blocking()` |
| `MV3D_LP_GetBatchProfile` | 未直接封装 | 废弃接口；功能替代为 `Device::get_image()` 或 `get_image_blocking()` |
| `MV3D_LP_GetIntensityData` | 未直接封装 | 废弃接口；亮度数据由 `Frame::intensity_data` 携带 |
| `MV3D_LP_RegisterProfileCallBack` | 未直接封装 | 废弃接口；功能替代为 `Device::start_receiving()` |
| `MV3D_LP_RegisterBatchProfileCallBack` | 未直接封装 | 废弃接口；功能替代为 `Device::start_receiving()` |
| `MV3D_LP_MapDepthToPointCloud` | `ImageProcessor::depth_to_point_cloud()` | internal FFI 校验输入与输出，返回 `Image` |
| `MV3D_LP_MapDepthToPointCloudRound` | `ImageProcessor::depth_to_round_point_cloud()` | 头文件规定最多 8 张；其他数量语义由 SDK 判断 |
| `MV3D_LP_ImageConvert` | `ImageProcessor::convert()` | 图像 descriptor 与布局校验集中在 internal FFI |
| `MV3D_LP_DepthMosaic` | `ImageProcessor::mosaic_depth()` | 头文件规定最多 8 张；其他数量语义由 SDK 判断 |
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
| `MV3D_LP_PARAM` | `Parameter`、`ParameterValue` | 通过 enum 取代 `ParamInfo` union 成员与判别字段；SDK 中不存在 `MV3D_LP_PARAM_INFO` 类型 |
| `MV3D_LP_EXCEPTION_INFO` | `DeviceException`、`DeviceExceptionType` | 拥有化描述并保留未知类型值 |
| `MV3D_LP_FILE_ACCESS` | `Device::download_file()`、`Device::upload_file()` | descriptor 只在调用期存在；成功后 `Device` 保留两段文件名 CString |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | `FileProgress` | 原样保留 signed `completed` 与 `total` |
| `MVB3D_LP_POINT_XYZ_S16`、`MVB3D_LP_POINT_XYZ_F32` | `Image`（`Frame` 为别名）的字节载荷 | 废弃数据；不公开单点结构体 |
| `MV3D_LP_PROFILE_DATA`、`MV3D_LP_DEPTH_DATA`、`MV3D_LP_INTENSITY_DATA` | `Frame` | 废弃数据；不直接映射旧采集结构体 |
| `MV3D_LP_POINTCLOUD_DATA` | `Image` | 废弃数据；点云统一表示为带 `ImageType` 的拥有化图像 |

SDK 的 reserved 字段、原始指针、回调函数指针和设备句柄只存在于私有 crate。

## 安全边界

- 公共 crate 使用 `#![forbid(unsafe_code)]`；FFI、指针校验、C union 读取和 callback trampoline 位于 `mv3d-lp-internal`。
- 原生图像输入与输出的判别值、指针、长度、布局和算术校验集中在 internal FFI，再复制到 Rust 所有值；校验依据实际 slice 和 SDK 长度字段，不设置任意的 512 MiB 上限。
- `Device` 独立持有 session 使用权；live owner 会阻止 Finalize。清理时如正在测量则调用一次 Stop，随后无论 Stop 结果都调用一次 Close；handle 在该 owner 上只提交一次 Close，不做理论性重试。
- FileAccess 的 `[IN]` 只表示参数方向，不能证明异步调用不持有字符串。启动成功后 `Device` 保存两段 CString；下一次成功 FileAccess 会替换它们，关闭或 Drop 时释放。
- `Sdk` 与 `ImageProcessor` 为 `Send + Sync`；`Device` 为 `Send + !Sync`，活动采集或传输可随设备的唯一所有权跨线程移动。
- callback registration 使用不复用的 cookie。撤销 registration 后，迟到 callback 查不到旧 cookie，不会命中新 registration；已取得 sink 的 callback 可以自行返回，撤销操作不等待它。
- callback 只向有界 Receiver 非阻塞入队；队列满时丢弃最新事件，Receiver 关闭后停止 Rust delivery。
- `Device` 仅用一个测量标记约束重复 Start 与无效 Stop；`soft_trigger`、`get_image`、参数、Execute、ClearDataBuffer 和 FileAccess 直接转发，由 SDK 返回调用顺序错误。
- 仍有 live owner 时 `shutdown()` 返回 `UnclosedDevices`；所有 `Device` owner 完成一次 Close 调用后才允许 Finalize。

safe API 依赖同步调用期间输入可读、SDK 输出在复制完成前有效，以及 Stop/Close 对资源的厂商契约。SDK、头文件、ABI 或固件变化后应重新审计相关接口。

## 待确认厂商契约

- `MV3D_LP_StopMeasure` 与 `MV3D_LP_CloseDevice` 返回时，image/exception callback 是否已静默。
- 同一 handle 是否支持重复注册 callback，以及跨 Stop、pull/callback 模式切换时旧 registration 的替换规则。
- FileAccess 是否同步复制文件名；确认后可缩短 `Device` 对 CString 的持有时间。
- FileAccess 的 signed `completed`、`total` 及 `(0, 0)` 的完成语义。
- Get/Set/Execute/ClearDataBuffer 在各设备状态下的允许矩阵，以及 Node Name 的编码、字符集和最大长度。

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
