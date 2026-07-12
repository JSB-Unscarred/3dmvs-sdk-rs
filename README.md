# mv3d-lp

海康威视 3D MVS 激光轮廓传感器 SDK 的安全 Rust 包装。项目把原始句柄、裸指针、C union、SDK 全局状态和资源清理封装在私有 crate 中，对外只提供安全 Rust API。

## 支持范围

- 原生目标：`x86_64-pc-windows-msvc`
- 已审计的 LPSDK 运行时版本：`1.3.3.3`
- Rust：`1.85` 或更高版本
- 默认启用 `native` feature

原生构建要求安装 3DMVS。构建脚本先读取 `MV3DLP_DEV_ENV`，未设置时使用：

```text
C:\Program Files (x86)\3DMVS\Development
```

它只链接 `Libraries\win64\Mv3dLp.lib`，不会复制厂商头文件、LIB 或 DLL，也不会修改 `PATH`。运行时环境通常由 3DMVS 安装程序配置；如果程序启动时报告 DLL 或 GenTL 加载失败，应确认以下 x64 目录优先于任何 `Win32_i86` 目录，并检查 `GENICAM_GENTL64_PATH`：

```text
C:\Program Files (x86)\Common Files\Mv3dLpSDK\Runtime\Win64_x64
C:\Program Files (x86)\Common Files\MV3D\Runtime\Win64_x64
```

非 x64 Windows MSVC 目标可使用 `--no-default-features` 编译安全门面；此时 `Sdk::initialize()` 返回 `Error::UnsupportedPlatform`。

## 基本使用

```rust,no_run
use std::net::Ipv4Addr;
use std::time::Duration;

use mv3d_lp::{ParamKey, ParameterValue, Result, Sdk};

fn main() -> Result<()> {
    let sdk = Sdk::initialize()?;
    println!("LPSDK {}", sdk.version());

    for device in sdk.devices()? {
        println!("{}", device.model_name.to_string_lossy());
    }

    let mut camera = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;
    camera.set_parameter(
        &ParamKey::new("ExposureTime")?,
        ParameterValue::Float(1000.0),
    )?;
    let mut measurement = camera.start()?;
    let frame = measurement.get_image(Duration::from_millis(100))?;
    println!(
        "frame {}: {}x{}, {} bytes",
        frame.frame_number,
        frame.width,
        frame.height,
        frame.data.len()
    );
    measurement.stop()?;
    camera.close()?;

    sdk.shutdown()
}
```

也可以使用 `SerialNumber` 和 `Sdk::open_by_serial` 打开设备。设备返回的文本使用 `SdkText` 保存原始有界字节；需要 UTF-8 时显式调用 `to_str()`，仅用于展示时可调用 `to_string_lossy()`。

`Sdk` 和 `Camera` 都不是 `Send` 或 `Sync`。每个进程只允许一次初始化尝试，`Finalize` 后不能重新初始化。`Camera` 借用 `Sdk`，因此相机存活时无法安全地关闭 SDK。

## 回调采集

```rust,no_run
use std::net::Ipv4Addr;

use mv3d_lp::{CallbackOptions, Result, Sdk};

fn receive_one_frame() -> Result<()> {
    let sdk = Sdk::initialize()?;
    let mut camera = sdk.open_by_ip(Ipv4Addr::new(192, 168, 1, 100))?;
    let (measurement, frames) = camera.start_receiving(CallbackOptions::default())?;

    if let Ok(frame) = frames.recv() {
        println!("callback frame {}, {} bytes", frame.frame_number, frame.data.len());
    }

    measurement.stop()?;
    camera.close()?;
    sdk.shutdown()
}
```

回调队列有界且使用非阻塞发送；队列满时丢弃最新事件。`start_with_callback` 和 `on_exception` 提供的闭包只在 Rust 工作线程串行执行，不会运行在 SDK callback 栈上。原生回调注册对单个设备句柄是一次性的；停止 callback measurement 后如需再次注册，应关闭并重新打开设备。

## 已提供

- 版本检查、初始化和结束
- 设备计数、带有限重试的设备枚举
- 按 IPv4 或序列号打开设备
- IP 配置
- Start、Stop、软触发、清空数据缓冲区和关闭
- Bool、Integer、Float、Enumeration、String 参数的读取与设置
- 命令节点执行
- `Measurement<'_>` 测量会话和 pull 模式图像获取
- `OwnedFrame` 主数据、可选亮度数据和可选逐行曝光时间戳的立即复制
- `CallbackMeasurement<'_>`、有界 `Receiver<OwnedFrame>` 图像回调和 Rust 工作线程闭包适配器
- 拥有化设备异常事件、未知异常值保留和同一 registry/cookie 生命周期保护
- 异步设备文件上传/下载、可恢复进度轮询和文件名生命周期保护
- `ImageProcessor` 深度转点云、环视点云、深度拼接、六种格式转换和图像保存
- `OwnedImage` 主数据及可用辅助载荷的立即复制
- 默认关闭的 Windows `display-windows` 图像显示 feature

当前不提供借用帧、零拷贝、废弃 API、原始句柄或 DLL 中未公开的导出。回调注册、晚到回调和并发排空契约记录在 [`m4/callback-contract.md`](m4/callback-contract.md)。

## 安全边界

- 公共 crate 使用 `#![forbid(unsafe_code)]`。
- 会执行 `unsafe` 操作的代码只位于私有 crate 的 FFI 文件；bindings 和 ABI 文件只保留原始声明及函数类型校验。
- 对外不公开厂商结构体、C union、裸指针或句柄。
- SDK 固定字符串会在字段边界内复制，且不假设编码。
- 参数 union 先校验判别值再读取；写入时整个结构先清零。
- 打开成功必须同时返回非空句柄；关闭调用后句柄无条件失效。
- Open 返回错误时附带的非空值不会被当作有效句柄再次传给 SDK。
- Start/Stop 失败后相机进入 `Faulted`，只允许清理。
- `Camera::start` 返回独占借用相机的 `Measurement`；显式 `stop` 可取得错误，`Drop` 最佳努力停止采集。
- GetImage 成功后会在私有 FFI 边界内校验描述符并立即复制所有可用载荷；SDK 裸指针不会跨越 Driver 边界。
- pull 采集只接受有限超时；`0xFFFF_FFFF` 无限等待哨兵不会传给 SDK。
- 单帧采用聚合内存上限和 fallible allocation，尺寸与长度运算全部使用 checked arithmetic。
- GetImage 依赖的 SDK 缓冲区稳定窗口和待补证据记录在 [`m2/image-ownership-contract.md`](m2/image-ownership-contract.md)。
- 图像和异常回调只把永不复用的 cookie 交给 SDK；Stop/Close 前先从 Rust registry 摘除并排空，用户闭包仅在 Rust 工作线程执行。
- 回调 payload 在 trampoline 返回前的稳定读取假设和剩余厂商问题记录在 [`m4/callback-contract.md`](m4/callback-contract.md)。
- 文件传输、ImgProc 输出所有权和显示句柄契约记录在 [`m3/contracts.md`](m3/contracts.md)。
- ImgProc 的 SDK 输出在同一把全局锁内校验并立即复制；多图操作只接受厂商规定的 1 至 8 张输入。
- 活动文件传输的两个文件名由 `Camera` 持有；提前丢弃 guard 不会释放文件名或假装取消传输。
- `Drop` 最佳努力执行清理且不 panic；显式 `close`/`shutdown` 可取得错误。
- 只有活句柄计数归零且每次关闭结果都确定成功时才调用 `Finalize`；相机被遗忘或 Close 失败时会跳过 Finalize，安全地泄漏原生会话直到进程退出。
- 所有 SDK 调用通过同一把进程级互斥锁串行化。

## 验证

无需安装 SDK 或连接相机：

```powershell
cargo fmt --all -- --check
cargo check --workspace --no-default-features
cargo clippy --workspace --all-targets --no-default-features -- -D warnings
cargo test --workspace --no-default-features
```

已安装 3DMVS 的 x64 Windows MSVC 主机可额外检查原生构建：

```powershell
cargo check --workspace --features native
cargo test --workspace --features native --no-run
cargo test --workspace --features display-windows
```

仓库中的测试只验证本包装的状态机、转换、边界和清理逻辑，不调用厂商 SDK，也不要求真实设备。
