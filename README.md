# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK 的 Rust 包装。在下述原生契约假设成立时，公共 crate 对外提供 safe Rust API，并将句柄、裸指针、C union、进程级状态和资源清理封装在私有 crate 中。

## 支持与安装

- 原生目标：`x86_64-pc-windows-msvc`
- 绑定与 strict 模式基线：LPSDK `1.3.3.3`
- 默认 ABI 兼容范围：`>=1.3.3.3, <1.3.4.0`
- MSRV：Rust `1.85`
- 默认 feature 集为空；构建和测试不需要厂商 SDK
- `native` 启用原生后端；`display-windows` 同时启用 `native` 和 Win32 图像显示

默认构建提供核心类型与 API，但不链接厂商库；`Sdk::initialize()` 会返回 `Error::UnsupportedPlatform`。当前两个 crate 均为 `publish = false`，因此应从 Git 仓库依赖：

```toml
[dependencies]
mv3d-lp = { git = "https://github.com/JSB-Unscarred/3dmvs-sdk-rs.git", features = ["native"] }
```

`native` 要求安装 3DMVS。构建脚本从 `MV3DLP_DEV_ENV` 读取 Development 目录，默认值为：

```text
C:\Program Files (x86)\3DMVS\Development
```

脚本只校验并链接 `Libraries\win64\Mv3dLp.lib`；checked-in bindings 不在构建时重新生成，也不要求公开头文件存在。脚本不会复制厂商文件或配置运行时环境。DLL 或 GenTL 加载失败时，请检查 `PATH`、`GENICAM_GENTL64_PATH`，并确保下列 x64 目录优先于 `Win32_i86`：

```text
C:\Program Files (x86)\Common Files\Mv3dLpSDK\Runtime\Win64_x64
C:\Program Files (x86)\Common Files\MV3D\Runtime\Win64_x64
```

其他系统、Windows GNU、32 位 Windows 和其他架构只能使用默认的无原生构建。

## 快速开始

```rust,no_run
use std::net::Ipv4Addr;
use std::time::Duration;

use mv3d_lp::{ParamKey, ParameterValue, Result, Sdk};

fn main() -> Result<()> {
    let sdk = Sdk::initialize()?;
    println!("LPSDK {}", sdk.version());

    for info in sdk.devices()? {
        println!("{}", info.model_name.to_string_lossy());
    }

    let mut device = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;
    device.set_parameter(
        &ParamKey::new("ExposureTime")?,
        ParameterValue::Float(1000.0),
    )?;

    let mut measurement = device.start()?;
    let frame = measurement.get_image(Duration::from_millis(100))?;
    println!(
        "frame {}: {}x{}, {} bytes",
        frame.frame_number,
        frame.width,
        frame.height,
        frame.data.len()
    );
    measurement.stop()?;
    device.close()?;
    sdk.shutdown()
}
```

也可使用 `SerialNumber` 和 `Sdk::open_by_serial`。SDK 返回的文本由 `SdkText` 保留原始有界字节；需要 UTF-8 时使用 `to_str()`，仅展示时可使用 `to_string_lossy()`。

`Sdk` 有意保持 `!Send + !Sync`，初始化与 `Finalize` 必须留在 owner thread。`Device`、`Measurement`、`CallbackMeasurement` 和 `FileTransfer` 是 `Send + !Sync`：`Send` 只允许把唯一所有权交给另一线程，不表示可以从多个线程并发调用同一句柄。

`Device` 仍借用 `Sdk`，设备存活时不能关闭 SDK；这个借用通常也不满足普通 `std::thread::spawn` 所要求的 `'static`。短期直接 handoff 应使用 `std::thread::scope`，长期 owner thread 则应在线程内部创建并依次关闭 `Sdk`、`Device` 和采集会话。

进程级 LPSDK 生命周期与单设备的 `DeviceState` 相互独立。内部 `ProcessSdkState` 只有 `Fresh`、`Active` 和 `Degraded` 三态：`Fresh` 允许创建一个 runtime，初始化成功后进入 `Active`；版本查询或兼容性检查失败仍为 `Fresh`，`Initialize` 失败且后续清理成功、以及成功 `Finalize` 后也会回到 `Fresh`。原生清理或 `Finalize` 的结果不确定，或者 runtime owner 在仍有 Rust 跟踪句柄时被终结，都会进入 `Degraded`。`Degraded` 表示进程级生命周期已不能安全扩张、收尾或重启；它不会使仍存活的设备、会话、文件传输或纯图像处理失效，但会永久拒绝新设备 open、`Finalize` 和后续 runtime 初始化。

### 回调采集

```rust,no_run
use mv3d_lp::{CallbackOptions, Device, Result};

fn receive_one_frame(device: &mut Device<'_>) -> Result<()> {
    let (measurement, frames) = device.start_receiving(CallbackOptions::default())?;

    if let Ok(frame) = frames.recv() {
        println!("callback frame {}, {} bytes", frame.frame_number, frame.data.len());
    }

    measurement.stop()
}
```

回调队列有界并采用非阻塞发送，队列满时丢弃最新事件。容量由 `CallbackOptions` 设置，默认值为 4；每个排队帧拥有其 payload，调用者应结合帧大小、帧率和消费延迟选择容量。`start_with_callback` 和 `on_exception` 的用户闭包各自在独立 Rust 工作线程中串行执行，不在 SDK callback 栈上运行。callback measurement 正常停止后，可在同一设备句柄上用新 cookie 重新注册图像回调；异常回调也可由新注册替换。若 SDK 不支持再次注册，原生错误会直接返回。

## 功能与限制

- 设备枚举、IPv4/序列号打开和 IP 配置
- pull/callback 采集、软触发、参数读写、命令执行和设备异常
- 拥有化帧及可选的亮度数据和逐行曝光时间戳
- 文件上传、下载和借用式进度轮询
- 深度转点云、环视点云、拼接、格式转换和图像保存
- 可选的 Win32 图像显示

当前采集结果不提供借用帧或零拷贝，也不公开原始句柄、废弃 API 或 DLL 未公开导出。

## 安全模型

- 公共 crate 使用 `#![forbid(unsafe_code)]`，不公开厂商结构体、C union、裸指针或句柄。
- 生产代码中的原生调用、C union 读取和裸指针解引用集中在私有 crate 的 [`ffi.rs`](mv3d-lp-internal/src/ffi.rs) 与 [`callback.rs`](mv3d-lp-internal/src/callback.rs)：前者负责原生调用和数据转换，后者负责 callback trampoline 与回调指针准入。`bindings.rs` 保存原始声明，`abi.rs` 校验布局和函数类型。
- SDK 数据先校验适用的指针、长度、判别值和 checked arithmetic，再复制为 Rust 所有值；大型图像载荷另受聚合上限和可失败分配保护。
- 生命周期和独占借用约束 SDK、设备、测量与文件传输的使用顺序；活动文件传输独占借用设备，guard 被丢弃后可重新取得并继续轮询；原生文件传输启动失败后设备只允许清理。
- 回调 cookie 永不复用；晚到或已撤销的回调被忽略；公共 API 的用户 handler 不在原生回调线程执行，unwind panic 被隔离在原生 ABI 边界内。
- 资源 `Drop` 做最佳努力清理；显式 `stop`、`close` 和 `shutdown` 返回清理错误。
- 有活句柄或清理结果不确定时跳过 `Finalize`；单个设备 Close 结果不确定会把进程级会话降级为 `Degraded`，阻止新设备、`Finalize` 和新 runtime，但不会改变其他设备各自的 `DeviceState`，也不会使它们的存量操作或纯图像处理失效。
- 进程级互斥锁只保护 runtime 生命周期和设备 open/close 记账；不同设备的普通调用可以并行。无设备句柄隔离的 ImgProc/Save 调用使用独立互斥锁串行化。

## 原生契约假设

安全 API 的 bindings 基于闭源 LPSDK `1.3.3.3` 审计，默认接受 `>=1.3.3.3, <1.3.4.0` 的同 ABI 修订；`Sdk::initialize_strict()` 可要求精确基线。公开头文件、官方多线程示例和唯一所有权模型支持当前 `Send + !Sync` 契约；基线版本的跨线程 handoff、析构和 callback drain 实机测试仍作为发布前运行验证，不作为 public auto trait 的启用门槛。

| 区域 | 必要的原生行为 | Rust 侧缓解 |
| --- | --- | --- |
| pull `GetImage` | 成功返回后，描述符和载荷在同步复制完成前可读且不被并发修改 | 由 `Device` 的唯一所有权排除同句柄并发，并在返回前校验指针、长度和算术后立即复制 |
| 图像回调 | trampoline 返回前，描述符和载荷保持可读且不被并发修改 | 在 trampoline 内校验并拥有化数据；公共 API 不在此执行用户 handler |
| 回调生命周期 | Stop/Close 未必排空回调，且 SDK 没有已文档化的注销操作 | registry 先撤销再排空；cookie 永不复用；晚到回调被忽略 |
| 文件传输 | `MV3D_LP_FILE_ACCESS` 输入描述符只在启动调用期间读取；后台至多保留其中的字符串指针；只有 `completed == total > 0` 表示完成 | 描述符保持调用期有效；文件名只保留到观察到完成或设备清理，之后立即释放且不累计历史；每个进度快照独立校验，不额外假定跨轮询单调或总量固定；若发现 SDK 保留描述符本身，则停用并重新设计该 safe API |
| ImgProc/Save | 输入不被写入或保留；输出在立即复制期间可读 | 仅传递调用期借用，并在独立 ImgProc 锁内校验结构与大小、复制输出；标定浮点值原样传递，不增加厂商未声明的 finite 策略 |
| Windows 显示 | 图像和 Win32 句柄只在同步调用期间被借用 | 通过 `raw-window-handle` 借用，并保持参数在调用期间存活 |

若新的文档、日志或实验与任一假设冲突，相关 safe API 必须停用或重新设计。LPSDK、头文件、导入库、DLL、运行时配置、ABI、固件或 FFI surface 变化后也必须重新审计。

## 开发与验证

无需安装 SDK 或连接设备：

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked
cargo doc --workspace --no-deps --locked
```

已安装 3DMVS 的 x64 Windows MSVC 主机可额外检查：

```powershell
cargo check --workspace --features native
cargo test --workspace --features native --no-run
cargo test --workspace --features display-windows
```

默认测试使用 fake backend 和本地 FFI stubs，不调用厂商 SDK，也不要求设备；`cargo test` 同时运行两个 crate 和负向编译契约。原生构建、ABI 复验和实机观察属于独立证据。

## 安全报告与许可证

请通过 [GitHub Security Advisories](https://github.com/JSB-Unscarred/3dmvs-sdk-rs/security/advisories/new) 私下报告内存安全、FFI、回调生命周期、ABI 或资源清理问题，并附上版本、feature、target、SDK/固件环境、已脱敏的最小复现和日志。

本项目采用 [MIT License](LICENSE)。许可证只覆盖本仓库代码，不授予 3DMVS/LPSDK 头文件、导入库、DLL、运行时、安装程序、设备或固件的再分发权。
