# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK 的 Rust 包装。在下述原生契约假设成立时，公共 crate 对外提供 safe Rust API，并将句柄、裸指针、C union、进程级状态和资源清理封装在私有 crate 中。

## 支持与安装

- 原生目标：`x86_64-pc-windows-msvc`
- 已审计的 LPSDK 运行时：`1.3.3.3`
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

脚本会校验公开头文件和 `Libraries\win64\Mv3dLp.lib`，但只链接导入库，不复制厂商文件或配置运行时环境。DLL 或 GenTL 加载失败时，请检查 `PATH`、`GENICAM_GENTL64_PATH`，并确保下列 x64 目录优先于 `Win32_i86`：

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

`Sdk` 有意保持 `!Send + !Sync`，初始化与 `Finalize` 必须留在 owner thread。`Device`、`Measurement` 和 `CallbackMeasurement` 是 `Send + !Sync`：`Send` 只允许把唯一所有权交给另一线程，不表示可以从多个线程并发调用同一句柄。

`Device` 仍借用 `Sdk`，设备存活时不能关闭 SDK；这个借用通常也不满足普通 `std::thread::spawn` 所要求的 `'static`。短期直接 handoff 应使用 `std::thread::scope`，长期 owner thread 则应在线程内部创建并依次关闭 `Sdk`、`Device` 和采集会话。原生 runtime 只允许一个活动实例；首次初始化失败或 `Finalize` 后进入终态，不能在同一进程中重试。

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

回调队列有界并采用非阻塞发送，队列满时丢弃最新事件。`start_with_callback` 和 `on_exception` 的用户闭包各自在独立 Rust 工作线程中串行执行，不在 SDK callback 栈上运行。同一设备句柄的图像与异常回调各自最多尝试注册一次；停止 callback measurement 后如需重新注册图像回调，必须关闭并重新打开设备。

## 功能与限制

- 设备枚举、IPv4/序列号打开和 IP 配置
- pull/callback 采集、软触发、参数读写、命令执行和设备异常
- 拥有化帧及可选的亮度数据和逐行曝光时间戳
- 文件上传、下载和拥有式进度轮询
- 深度转点云、环视点云、拼接、格式转换和图像保存
- 可选的 Win32 图像显示

当前采集结果不提供借用帧或零拷贝，也不公开原始句柄、废弃 API 或 DLL 未公开导出。

## 安全模型

- 公共 crate 使用 `#![forbid(unsafe_code)]`，不公开厂商结构体、C union、裸指针或句柄。
- 生产代码中的原生调用、C union 读取和裸指针解引用集中在私有 crate 的 [`ffi.rs`](mv3d-lp-internal/src/ffi.rs) 与 [`callback.rs`](mv3d-lp-internal/src/callback.rs)：前者负责原生调用和数据转换，后者负责 callback trampoline 与回调指针准入。`bindings.rs` 保存原始声明，`abi.rs` 校验布局和函数类型。
- SDK 数据先校验适用的指针、长度、判别值和 checked arithmetic，再复制为 Rust 所有值；大型图像载荷另受聚合上限和可失败分配保护。
- 生命周期和所有权约束 SDK、设备、测量与文件传输的使用顺序；活动文件传输拥有设备，只有观察到完成后才能取回；Start/Stop 失败后设备只允许清理。
- 回调 cookie 永不复用；晚到或已撤销的回调被忽略；公共 API 的用户 handler 不在原生回调线程执行，unwind panic 被隔离在原生 ABI 边界内。
- 资源 `Drop` 做最佳努力清理；显式 `stop`、`close` 和 `shutdown` 返回清理错误。
- 有活句柄或清理结果不确定时跳过 `Finalize`，原生会话保守地保留到进程退出。
- 所有 SDK 调用通过同一把进程级互斥锁串行化。

## 原生契约假设

安全 API 依赖闭源 LPSDK `1.3.3.3` 的下列行为。公开头文件和官方示例与之相符，但这些行为没有独立的厂商书面保证；跨线程 handoff、析构和 callback drain 的锁定版本实机验收仍待完成，在通过前不得把 public auto trait 的编译能力视为已取得原生发布资格。

| 区域 | 必要的原生行为 | Rust 侧缓解 |
| --- | --- | --- |
| pull `GetImage` | 成功返回后，描述符和载荷在同步复制完成前可读且不被并发修改 | 在进程锁内校验指针、长度和算术后立即复制 |
| 图像回调 | trampoline 返回前，描述符和载荷保持可读且不被并发修改 | 在 trampoline 内校验并拥有化数据；公共 API 不在此执行用户 handler |
| 回调生命周期 | Stop/Close 未必排空回调，且 SDK 没有已文档化的注销操作 | registry 先撤销再排空；cookie 永不复用；晚到回调被忽略 |
| 文件传输 | Close 成功会终止后台访问；只有 `completed == total > 0` 表示完成 | 活动及已完成传输的文件名均保留到成功 Close；Close 结果不确定时保守泄漏 |
| ImgProc/Save | 输入不被写入或保留；输出在立即复制期间可读 | 仅传递调用期借用，并在全局锁内校验、限制大小和复制输出 |
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
