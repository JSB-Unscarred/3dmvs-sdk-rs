# 3dmvs-sdk-rs 项目指南

本文件适用于整个仓库。若将来某个子目录增加更具体的 `AGENTS.md`，则该文件只覆盖其所在子树；未被覆盖的规则仍以本文件为准。

## 项目概述

本项目是海康威视 3D MVS 激光轮廓传感器 LPSDK 的 Rust 包装。目标是在已审计的原生契约成立时，对外提供 safe、符合 Rust 所有权模型的 API，并把句柄、裸指针、C union、进程级状态和资源清理封装在私有实现层。

- Cargo workspace 使用 resolver 3、Rust 2024 edition，MSRV 为 Rust 1.85。
- workspace 包含公共 crate `mv3d-lp` 和私有 crate `mv3d-lp-internal`；两者当前均为 `publish = false`。
- checked-in bindings 与 strict 模式基线是 LPSDK `1.3.3.3`；默认初始化接受 `>=1.3.3.3, <1.3.4.0` 的兼容版本。
- 默认 feature 为空，不链接或加载厂商 SDK，也不要求设备。
- `native` 启用原生后端，只支持 `x86_64-pc-windows-msvc`。
- `display-windows` 隐含启用 `native`，并提供基于 `raw-window-handle` 的 Win32 图像显示。

## 仓库结构

```text
.
├── Cargo.toml                         # workspace 与公共 crate mv3d-lp
├── src/                               # 完全 safe 的公共 API 门面
├── tests/                             # 公共 API、类型、线程和负向编译契约
├── mv3d-lp-internal/
│   ├── Cargo.toml                     # 私有实现 crate
│   ├── build.rs                       # 原生目标检查与 Mv3dLp.lib 链接
│   ├── src/                           # bindings、FFI、状态机、回调和资源管理
│   └── tests/native_thread_contract.rs # ignored 的实机线程契约验收
├── docs/threading/                    # 原生线程契约的发布验证记录
├── README.md                          # 用户文档、功能边界和快速开始
└── target/                            # Cargo 生成物，不编辑、不提交
```

## 分层架构

### 公共 crate：`mv3d-lp`

`src/lib.rs` 使用 `#![forbid(unsafe_code)]`。各领域模块保持私有，由 crate 根显式重导出公共类型。公共层负责 Rust 风格的类型与生命周期、调用前输入校验，以及把内部拥有化记录转换为公开值；不得暴露厂商结构体、C union、裸指针、原始句柄或内部 crate 的实现细节。

| 文件 | 职责 |
| --- | --- |
| `src/sdk.rs` | `Sdk` 生命周期、设备发现/打开与图像处理器创建 |
| `src/opened_device.rs`、`callback.rs`、`file_transfer.rs` | 设备状态、采集/传输 guard、回调通道与用户工作线程 |
| `src/frame.rs`、`image_processor.rs`、`display_windows.rs` | 图像类型、布局校验、处理、保存及可选 Windows 显示 |
| `src/device.rs`、`parameter.rs`、`text.rs`、`types.rs` | 拥有化设备、参数、文本和版本类型 |
| `src/error.rs` | 公共错误模型、状态码和内部错误映射 |

### 私有 crate：`mv3d-lp-internal`

该 crate 是唯一允许接触 unsafe 与原生 ABI 的实现边界，不对直接使用者作兼容承诺。

| 文件 | 职责 |
| --- | --- |
| `bindings.rs`、`abi.rs` | checked-in C 声明与 Windows x64 MSVC ABI 断言；构建时不运行 bindgen |
| `driver.rs`、`ffi.rs` | native/mock 后端边界、原生调用、C union、指针校验与数据复制 |
| `runtime.rs`、`opened_device.rs` | 进程/设备状态机、句柄记账、资源 guard 与清理顺序 |
| `callback.rs` | 全局 registry、cookie、trampoline、回调撤销和在途 drain |
| 其余领域模块 | 跨 safe driver 边界的拥有化记录、错误与 Windows 显示记录 |

生产后端由 `ffi.rs` 中的 `NativeDriver` 实现；默认测试由 `mv3d-lp-internal/src/tests/mock_driver.rs` 注入 fake driver，因此状态机、错误和清理路径必须继续能够在无 SDK 环境验证。

## 关键数据流

```text
调用者
  → mv3d_lp 公共类型、输入校验和借用关系
  → mv3d_lp_internal Runtime / Device 状态机
  → Driver trait
  → NativeDriver
  → bindings 中的 MV3D_LP_* 原生函数
```

返回数据按相反方向流动。失败后的结构化输出和 payload 一律不可信；open 仅可检查输出 handle 是否非空，以识别不确定状态并进入 `Degraded`，不得使用该句柄。成功输出经过指针、长度、判别值、算术和分配上限校验后立即复制为拥有值；公开 owned 输出不得继续借用 SDK 内存。

### 生命周期与线程模型

```text
Sdk / internal Runtime
├── Device<'sdk>
│   ├── Measurement<'device>
│   ├── CallbackMeasurement<'device>
│   └── FileTransfer<'device>
└── ImageProcessor<'sdk>
```

- `Sdk` 与 `ImageProcessor` 是 `!Send + !Sync`；初始化和 `Finalize` 留在 owner thread。
- `Device`、`Measurement`、`CallbackMeasurement` 和 `FileTransfer` 是 `Send + !Sync`。这只允许转移唯一所有权，不允许共享后并发调用同一句柄。
- `Device<'sdk>` 借用 `Sdk`；活动测量或文件传输 guard 独占借用 `Device`。不要用裸指针、泄漏或额外内部可变性绕过这些编译期约束。
- 短期跨线程 handoff 使用 `std::thread::scope`；长期 owner thread 应在线程内创建并依次关闭 `Sdk` 和设备资源。
- 不同设备的普通调用可以并行。进程锁只保护 runtime 生命周期与 open/close 记账；没有设备句柄隔离的 ImgProc/Save/Display 调用由独立锁串行化。
- 进程级 `Fresh / Active / Degraded` 与每个设备的 `DeviceState` 相互独立。`Degraded` 永久阻止新 open、`Finalize` 和重新初始化，但不应无故禁用仍存活设备的存量操作或纯图像处理。
- 版本查询或兼容性检查失败保持 `Fresh`。`Initialize` 失败后必须尝试补偿性 `Finalize`：补偿成功才保持 `Fresh`，补偿失败则进入 `Degraded`。

### 采集、回调与文件传输

- Pull 采集由 `Measurement` 独占设备；`GetImage` 返回后，描述符及所有 payload 必须在返回公共层前校验并拥有化。
- 原生图像 callback 经 trampoline 和永不复用的 cookie 查找 registry，在原生 callback 返回前复制数据，再用有界 `sync_channel::try_send` 投递。队列满时丢弃最新事件。
- 用户 handler 只在 Rust 工作线程串行执行，不得在 SDK callback 栈上运行。
- 停止 callback measurement 时先撤销 registry admission、等待已接纳 callback drain，再调用原生 Stop；迟到或已撤销 callback 只能无操作返回。
- 文件传输开始后设备进入 `Transferring`。文件名的 `CString` 至少保留到观察到 `completed == total > 0` 或设备被清理。
- 丢弃 `FileTransfer` guard 不等于取消原生传输；使用 `Device::active_file_transfer()` 恢复轮询。

## 设计原则与官方证据

- 安全设计以官方开发文档、头文件和示例为依据，目标是实现足以保证 safe API soundness 的最小机制，并保持实现简洁、清晰和优雅。不得脱离官方材料自由发挥，或仅因某个行为未被逐字说明就堆叠额外锁、状态机、复制、泄漏策略和故障域。
- 官方文档与头文件中的明确约束是硬边界；未直接说明的调用顺序、线程或生命周期行为，可以结合官方示例作合理推断。推断只覆盖示例实际展示的场景，不扩张为任意并发或无限生命周期保证；重要推断应记录依据和适用范围。
- 证据不足时，先选择更小的 API、局部校验或定向验证。只有能指出具体风险、官方依据或可复现失败模式，并说明更轻方案为何不足时，才增加新的安全机制。
- 优先使用 Rust 所有权、借用、RAII、类型状态和局部输入校验表达约束；同步与进程级状态只保护确有共享或原生生命周期需求的最小范围。
- 官方材料之间冲突时，不自行猜测：以明确契约和 ABI 声明为先，收窄相关能力并记录待确认点。

官方 Development 根目录由 `MV3DLP_DEV_ENV` 指定，默认是 `C:\Program Files (x86)\3DMVS\Development`。审计与设计时使用：

| 材料 | 默认路径 |
| --- | --- |
| 开发文档 | `C:\Program Files (x86)\3DMVS\Development\Documentations` |
| 官方示例 | `C:\Program Files (x86)\3DMVS\Development\Samples` |
| 官方头文件 | `C:\Program Files (x86)\3DMVS\Development\Includes` |

## 必须保持的安全不变量

1. 公共 crate 始终保持 `#![forbid(unsafe_code)]`；所有 FFI、裸指针、C union 和句柄操作留在私有 crate。
2. 失败后不读取结构化输出或 payload；open 仅按已审计规则检查输出 handle 是否非空。成功后先校验判别值、指针/长度、算术、大小和分配，再解引用或构造 slice。
3. 原生输入的 reserved 字段和 union 非活动存储清零；读取 union 前验证 discriminator。
4. SDK 文本和文件名保留原始有界字节；参数/命令 key 沿用非空 ASCII 约束，未知 SDK 数值保留原始值。
5. trampoline 阻止 panic 穿过 ABI；`CallbackWorker` 单独隔离用户 handler panic。cookie 不复用，撤销 registry 时等待 in-flight callback drain。
6. 资源按依赖顺序创建、逆序清理；清理步骤互不剥夺机会。显式清理返回错误，`Drop` 只做 best-effort 且不 unwind。
7. 初始化或清理结果不确定时进入 `Degraded`；有活句柄时不得 `Finalize`。版本查询/兼容性失败保持 `Fresh`，`Initialize` 失败只有在补偿性 `Finalize` 成功后才保持 `Fresh`。
8. 不得在缺少厂商契约、soundness 论证、auto-trait 测试和原生验收计划时扩大 `unsafe impl Send/Sync`。
9. 公共层与 FFI 层的双重校验属于纵深防御。修改 FFI surface、ABI、SDK 版本范围、线程或资源生命周期假设时，重新审计文档和实机验收。

## 编码规范

- 使用 `cargo fmt` 默认格式和 Rust 命名约定。模块按领域划分；公共 API 仅从 `src/lib.rs` 精确重导出，内部项默认保持 `pub(crate)`。
- 代码标识符、Rustdoc 与源码注释沿用现有英文风格；仓库说明可使用中文。注释解释约束、所有权或安全理由，不复述代码。
- 公开 API 变更应优先保持向后兼容。可扩展的 SDK 数值优先使用保留未知值的 newtype；适合扩展的公开 enum/struct 使用现有 `#[non_exhaustive]` 习惯。
- 可恢复错误返回结构化 `Result` 并使用 `?` 传播；保留原始 SDK status、operation 和契约上下文。
- 生产路径不得用 `unwrap()`/`expect()` 处理外部输入、SDK 输出、状态码、锁中毒或清理结果。仅当不变量已由本地代码证明时才可使用带精确说明的 `expect()`；测试代码可按需使用。
- 浮点比较必须显式定义 NaN 语义，不得直接对 `partial_cmp()` 使用 `unwrap()`。
- 涉及用户/SDK 输入且可能溢出或截断的长度、计数和字节数，先用 checked arithmetic 校验并优先使用 `TryFrom`；已由局部不变量证明安全的扩宽或位模式转换可保留显式 `as`。始终在分配或建立 slice 前验证边界。
- 避免不必要的大对象复制，但不要为机械追求“零拷贝”破坏当前借用/owned 边界。`OwnedFrame`、`OwnedImage` 故意不实现 `Clone`。
- `mv3d-lp-internal/src/lib.rs` 的 `unsafe_op_in_unsafe_fn`、`undocumented_unsafe_blocks` 和 improper-ctypes lint 不得放宽。每个 unsafe 块保持最小，并在紧邻位置用 `// SAFETY:` 说明前置条件。
- `bindings.rs` 可集中容纳厂商 C 命名及必要 lint 例外；不要把这些 `allow` 扩散到普通模块。
- 新增原生函数必须贯穿 bindings、ABI、`Driver`/`NativeDriver`、mock/failpoint、拥有化记录、safe API、错误映射和文档。
- 新增或修改资源 guard 时使用借用表达调用顺序，并为重要 Drop 语义保留 `#[must_use]` 与 Rustdoc。

## 修改导航与测试映射

| 变更 | 主要位置 | 必须同步检查 |
| --- | --- | --- |
| 公共 API、类型、重导出 | `src/<领域>.rs`、`src/lib.rs` | `tests/public_api.rs`、`tests/thread_traits.rs`、README，必要时 compile-fail |
| 生命周期、设备状态、清理 | `sdk.rs`、两层 `opened_device.rs`、内部 `runtime.rs` | runtime/device/measurement/cleanup 测试和生命周期 compile-fail |
| Callback 或文件传输 | 两层 callback/file-transfer/opened-device、内部 `ffi.rs` | callbacks/threading/file-transfer 测试和原生线程契约 |
| 帧、图像处理、参数或文本 | 对应公共模块、内部记录/`ffi.rs` | image/parameter/string 测试，裸指针变化时追加 Miri |
| 错误/status 映射 | 两层 `error.rs`、`driver.rs` | `tests/error_mapping.rs` 与 failure/failpoint ledger |
| bindings、ABI 或新原生调用 | `bindings.rs` → `abi.rs` → `driver.rs` → `ffi.rs` → safe 门面 | mock driver、失败注入、ABI、public API 与文档 |
| build.rs、feature、Windows 显示或 SDK 基线 | 两份 `Cargo.toml`、`build.rs`、相关模块 | build-script、feature、README、线程契约和原生验证 |

负向编译测试位于 `tests/compile_fail.rs` 和 `tests/compile_fail_cases/`，并固定检查 rustc error code。借用或 auto-trait 变化时应更新或新增准确用例；不能为了让测试通过而删除契约测试或放宽预期错误。

## 构建与验证

所有命令从仓库根目录执行。默认完整软件门禁不需要厂商 SDK 或设备：

```powershell
cargo fmt --all -- --check
cargo check --workspace --locked
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --locked
cargo doc --workspace --no-deps --locked
```

- 默认测试包括公共集成测试、两个 crate 的单元测试、本地 FFI stubs 和负向编译契约。
- 只改文档时至少检查链接、路径、命令和示例仍与代码一致；Rust/API 变更在交付前运行完整五项门禁。
- 公共 API、生命周期或 auto trait 可先定向运行 `cargo test --test public_api --test thread_traits --test compile_fail --locked`。
- FFI、状态机、回调或清理变更可先运行 `cargo test -p mv3d-lp-internal --lib --locked`，随后仍执行完整门禁。
- 修改依赖时同步更新并提交 `Cargo.lock`；日常验证保留 `--locked`，避免静默改锁文件。

安装 3DMVS 的 x64 Windows MSVC 主机额外运行：

```powershell
cargo check --workspace --features native --locked
cargo test --workspace --features native --no-run --locked
cargo test --workspace --features display-windows --locked
```

`mv3d-lp-internal/build.rs` 从 `MV3DLP_DEV_ENV` 读取 Development 根目录，默认是 `C:\Program Files (x86)\3DMVS\Development`，只校验并链接 `Libraries\win64\Mv3dLp.lib`。脚本不复制 DLL、不设置 `PATH`/GenTL 环境，也不重新生成 bindings。原生目标编译或链接成功不等于真实 DLL、固件和设备行为已经通过。

涉及裸指针或 FFI 数据复制时，可追加固定工具链的纯 Rust Miri 验证：

```powershell
cargo +nightly-2026-07-09 miri test -p mv3d-lp-internal --lib --no-default-features --locked
```

实机线程契约测试是发布验证，不是默认测试或 public trait 的启用门槛。只有在明确准备好 LPSDK 1.3.3.3、在线设备、只读 device fixture 和预先存在的空 ASCII scratch 目录时，才按 `docs/threading/lpsdk-1.3.3.3-native-acceptance.md` 执行 ignored 测试。不得把设备序列号、fixture 名称、完整本地路径或完整原始日志提交到仓库。

## 文档维护

- 文档只描述当前设计与可验证事实，不记录阶段性 commit 或已过期计划。
- 用户可见行为变化时同步 `README.md`；架构、安全契约或验证方式变化时同步本文件；ABI/线程契约变化时同步 `docs/threading/`。
- 原生编译、mock/stub 测试和实机行为必须分开表述，不得相互替代。

## Git 与提交信息

- 主分支为 `main`。保留用户已有的未提交改动，不执行破坏性的 reset/checkout，也不把无关变更混入当前任务。
- 一个 commit 只表达一个逻辑变更。除非用户明确要求，否则不要自动创建 commit。
- commit 标题使用 `<type>(<scope>)?!: <中文简述>`；scope 可选，统一使用英文小写 type、半角冒号和一个空格，标题末尾不加句号。
- 常用 type：`feat`、`fix`、`refactor`、`test`、`docs`、`chore`、`perf`。破坏性变更在 type/scope 后加 `!`。
- 标题描述行为变化，不写“更新代码”等空泛内容。复杂提交在空行后的正文说明动机、关键约束、兼容性和验证结果,要有分点的正文.
