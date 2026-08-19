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

fn main() {
    if let Err(error) = run() {
        eprintln!("fatal: {error}");
        std::process::abort();
    }
}

fn run() -> Result<()> {
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

该示例采用终止式错误策略。普通 SDK status、输入、状态和同步输出契约错误先通过
`Result` 离开 `run()`；局部 owner 执行可完成的清理后，最终 binary 记录错误并结束进程。
库只在 callback ABI 无法返回错误的边界直接终止进程。

`Device::close()` 依次尝试必要的 Stop 和一次 Close。单一错误原样返回，两者同时失败时
返回 `Error::DeviceCleanup`。Close 失败后不重试：所有成功 FileAccess 的文件名 backing
封存至进程退出，进程级 Finalize 被阻止，调用方应停止 SDK 操作并结束进程。

设备也可通过 `SerialNumber` 与 `Sdk::open_by_serial()` 打开。SDK 文本使用 `SdkText` 保存原始字节，可按需调用 `to_str()` 或 `to_string_lossy()`。

需要无限等待下一帧时调用 `device.get_image_blocking()`；`get_image(Duration)` 只表达有限等待。

## 生命周期与时序

所有 session owner、Device 状态、清理顺序、callback、FileAccess、错误传播与终止边界集中在下列文档：

- [生命周期与时序图总览](生命周期与时序图.md)
- [标准生命周期与 pull 采集](时序图/标准生命周期与-pull-采集.md)
- [callback 采集与停止](时序图/callback-采集与停止.md)
- [文件上传与下载](时序图/文件上传与下载.md)

## SDK 接口对应表

下表严格依照 LPSDK `1.3.3.3` 的 `Mv3dLpApi.h` 与 `Mv3dLpImgProc.h` 声明顺序。已安装的 CHM 为 V1.3.2，缺少 `ClearDataBuffer` 和 FileAccess 接口，callback 的目录顺序也与较新头文件不同。调用顺序与清理约定见[生命周期与时序图](生命周期与时序图.md)。

| SDK 接口 | safe Rust 接口 | 说明 |
| --- | --- | --- |
| `MV3D_LP_GetVersion` | `Sdk::version()` | 不依赖 Initialize，返回 `SdkText` 原始字节；不解析版本段数 |
| `MV3D_LP_Initialize` | `Sdk::initialize()` | 初始化进程级 SDK |
| `MV3D_LP_Finalize` | `Sdk::shutdown(self)` | session owner 全部释放且未发生 Close failure 时调用；否则返回状态错误且不进入 native Finalize |
| `MV3D_LP_GetDeviceNumber` | `Sdk::device_count_hint()` | 返回枚举容量提示 |
| `MV3D_LP_GetDeviceList` | `Sdk::devices()` | 按一次 `GetDeviceNumber` 的结果调用一次；计数为 0 时直接返回空列表 |
| `MV3D_LP_OpenDeviceByIP` | `Sdk::open_by_ip()` | status 成功且 handle 非空后才创建 `Device` |
| `MV3D_LP_OpenDeviceBySN` | `Sdk::open_by_serial()` | 校验 `SerialNumber`；成功后才交付 handle |
| `MV3D_LP_CloseDevice` | `Device::close()`、`Device` 的 `Drop` | 必要时先 Stop，再调用一次 Close；显式关闭按单错误原样、双错误聚合返回；Close failure 阻止 Finalize |
| `MV3D_LP_SetIpConfig` | `Sdk::set_ip_config()` | 使用 `IpConfiguration` 表达配置模式 |
| `MV3D_LP_RegisterExceptionCallBack` | `Device::exception_receiver()`、`Device::disable_exception_delivery()` | 接收或停止 Rust 异常投递 |
| `MV3D_LP_StartMeasure` | `Device::start()`、`Device::start_receiving()` | 仅允许从 `Idle` 开始；pull 与 callback 状态互斥 |
| `MV3D_LP_StopMeasure` | `Device::stop()`、`Device::close()` 及 `Drop` | 仅停止运行中的采集；pull 成功回到 `Idle`，callback registration 保存至 Close |
| `MV3D_LP_SoftTrigger` | `Device::soft_trigger()` | 直接转发，由 SDK 判断调用顺序 |
| `MV3D_LP_GetImage` | `Device::get_image()`、`Device::get_image_blocking()` | pull 采集的有限等待与无限等待；internal FFI 校验后复制为 `Frame`（即 `Image`） |
| `MV3D_LP_RegisterImageDataCallBack` | `Device::start_receiving()` | 返回有界 `Receiver<Frame>`；Register 成功后该 handle 绑定 callback 至 Close |
| `MV3D_LP_ClearDataBuffer` | `Device::clear_buffer()` | 直接转发；允许状态待厂商确认 |
| `MV3D_LP_GetParam` | `Device::get_parameter()` | 接收 `&str` Node Name，返回 `Parameter` |
| `MV3D_LP_SetParam` | `Device::set_parameter()` | 接收 `&str` Node Name 与 `ParameterValue` |
| `MV3D_LP_Execute` | `Device::execute()` | 接收 `&str` Command Node Name |
| `MV3D_LP_FileAccessRead` | `Device::download_file()` | 下载设备文件；每次成功调用的文件名保存至 Close |
| `MV3D_LP_FileAccessWrite` | `Device::upload_file()` | 上传主机文件；每次成功调用的文件名保存至 Close |
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
| `MV3D_LP_FILE_ACCESS` | `Device::download_file()`、`Device::upload_file()` | descriptor 由 wrapper 构造；每次成功调用的两段 CString 保存至 Close |
| `MV3D_LP_FILE_ACCESS_PROGRESS` | `FileProgress` | 原样保留 signed `completed` 与 `total` |
| `MVB3D_LP_POINT_XYZ_S16`、`MVB3D_LP_POINT_XYZ_F32` | `Image`（`Frame` 为别名）的字节载荷 | 废弃数据；不公开单点结构体 |
| `MV3D_LP_PROFILE_DATA`、`MV3D_LP_DEPTH_DATA`、`MV3D_LP_INTENSITY_DATA` | `Frame` | 废弃数据；不直接映射旧采集结构体 |
| `MV3D_LP_POINTCLOUD_DATA` | `Image` | 废弃数据；点云统一表示为带 `ImageType` 的拥有化图像 |

SDK 的 reserved 字段、原始指针、回调函数指针和设备句柄只存在于私有 crate。

## 安全边界

- 公共 crate 使用 `#![forbid(unsafe_code)]`；FFI、指针校验、C union 读取和 callback trampoline 位于 `mv3d-lp-internal`。
- 原生图像输入与输出的判别值、指针、长度、布局和算术校验集中在 internal FFI，再复制到 Rust 所有值；已知格式要求主数据、可选亮度数据和曝光时间戳与宽高精确对应。
- 所有权、线程契约、清理顺序、callback、FileAccess、错误传播与终止边界统一见[生命周期与时序](生命周期与时序图.md)。

safe API 依赖同步调用期间输入可读、SDK 输出在复制完成前有效。SDK、头文件、ABI 或固件变化后应重新审计相关接口。

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
