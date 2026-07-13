# M5：硬化与发布

状态：代码与本地无 SDK 门禁已实现，等待托管 CI 的 MSRV/Miri 结果；registry 发布已延期，两个 crate 保持 `publish = false`  
目标版本：`0.1.0`  
Rust MSRV：`1.85`  
原生审计基线：`x86_64-pc-windows-msvc` / LPSDK `1.3.3.3`

## 1. 目标与边界

M5 不再扩大安全 API 范围，而是把 M1 至 M4 已实现的边界变成可重复、可审计的发布门禁：

- fake backend 为安全门面实际使用的每个 Driver 操作提供可定位的故障注入，并验证错误映射、状态迁移和资源失效；
- 对显式清理和 `Drop` 的调用顺序、跳过条件、幂等性和不 panic 契约做序列断言；
- 固定 Rust FFI 布局、常量、函数与回调调用约定，并保留厂商头文件探针的独立复验路径；
- 用 compile-fail 测试固定借用状态机，用 auto-trait 断言固定 `Send`、`Sync`、`Clone` 等边界；
- 用 Miri 运行不链接厂商库的纯 Rust 单元测试；
- 固定 feature、MSRV、平台、包元数据、发布顺序和回滚步骤。

所有 required CI 必须在没有 3DMVS、厂商头文件、导入库、DLL 和真实设备的托管 runner
上完成。安装了厂商 SDK 的构建、C++ ABI 探针和实机实验只能作为独立证据任务，不能成为
普通 pull request 的 required check，也不能把不可再分发的厂商文件上传为 CI artifact。

## 2. 发布面与 feature 决策

若未来明确授权 registry 发布，工作区将发布两个同版本 crate：

| crate | 角色 | 版本关系 |
| --- | --- | --- |
| `mv3d-lp` | 用户依赖的安全门面 | 发布版本 `0.1.0` |
| `mv3d-lp-internal` | 私有 FFI、状态机和清理实现 | 与门面同步，门面使用精确依赖 `=0.1.0` |

当前两个 manifest 均以 `publish = false` 防止误发布。解除该保护后，crates.io 不会把未发布
的 path dependency 嵌入上层包，因此必须先发布 internal crate、等待
索引可解析，再发布门面。internal crate 虽可从 registry 下载，但仍是实现细节；不承诺直接
使用它的 API 兼容性。

feature 策略固定如下：

| feature | 默认 | 作用 | 支持范围 |
| --- | --- | --- | --- |
| 空 feature 集 | 是 | 编译 API、运行 fake/纯 Rust 测试；初始化返回 `UnsupportedPlatform` | Rust 支持的平台 |
| `native` | 否 | 链接并调用 LPSDK | 仅 `x86_64-pc-windows-msvc`、运行时恰为 `1.3.3.3` |
| `display-windows` | 否 | 启用 `native` 和 Win32 窗口显示 | 同上 |

fake backend 只通过 `cfg(test)` 和 crate 内测试构造器注入，不成为公开 Cargo feature。这样既
不会给用户形成“可替换生产后端”的兼容承诺，也不会让测试钩子进入发布 API。

## 3. 自动化硬化门禁

### 3.1 fake backend 与清理顺序

故障覆盖分成两本不能互相替代的账本：

1. 默认无 SDK 单元测试为生产 `bindings` 使用的同名 C 符号提供 Rust stub，使测试实际经过
   `NativeDriver -> bindings::MV3D_LP_*`。raw 账本覆盖 28/28 个生产调用点（包括默认 feature
   不公开的 Display 共享调用 helper），校验实际符号、原始失败状态，并向带输出的调用写入
   畸形但已初始化的 out-param，证明失败 status 在描述符、指针、计数和 union 转换之前返回；
2. `MockDriver` 的统一 injector 覆盖无显示 Driver trait 的 `FfiOp::ALL` 27/27 项，以及
   `display-windows` 构建的 28/28 项，用来验证 post-FFI 状态机、错误映射和 Drop 清理。

两本账本都校验操作名唯一，并在新增生产调用或 Driver 方法却没有更新覆盖时失败。raw symbol
stub 只编入 `cfg(test)` 的无 native 单元测试，不是公开 fake feature，也不会绕过生产构建的
SDK 链接检查。每个可达操作至少验证：

OpenIP/OpenSN 是唯一特例：输出槽在调用前由 Rust 初始化为空；失败时只检查它是否被改成
非空，以便把 runtime 标记为 cleanup 不确定并跳过 Finalize。该值不会被解引用、构造成公开
Camera 或再次传给 Close。raw fake 同时覆盖“失败+空”和“失败+非空”两种返回。

1. 正常返回；
2. 对应 SDK 状态失败能保留原始状态码和操作名；
3. focused raw-conversion 测试对带输出的危险路径验证失败 status 优先、畸形输出拒绝和立即复制；
4. 失败后的状态只允许契约规定的恢复或清理动作；
5. 显式 `stop`、`close`、`shutdown` 能返回清理错误，`Drop` 吞掉错误且不 panic；
6. 句柄一旦传给 Close 即在 Rust 侧失效，不因 Close 失败而二次使用；
7. callback cookie 先撤销并排空，再 Stop/Close；文件名和其他可能仍被 SDK 保留的输入按
   契约延长或有意泄漏；Finalize 只在活句柄归零且清理结果确定时发生。

测试失败信息必须带操作名和完整调用日志，使新增 Driver 方法却没有故障用例时容易定位。
覆盖账本应与 Driver trait 同一变更提交更新；只比较总数不足以替代逐操作语义断言。

### 3.2 ABI

required 的无 SDK ABI 测试负责：

- 对公开绑定的大小、对齐和关键字段偏移与 `m0/abi/x64.json` 保持一致；
- 固定所有公开状态码、枚举位模式和数量上限；
- 固定导出函数为 `extern "C"`，回调为 `extern "system"`；
- 对 32 位与 64 位差异只作基线审计，不把 x86 宣称为当前 Rust 原生支持目标。

Rust 常量断言只能证明“当前声明与仓库快照一致”。接受新的 SDK 或准备正式 native 发布时，
还必须在受控 Windows 主机上用厂商头文件、对应架构 MSVC 和导入库运行 C++ ABI 探针，
逐字段比较新输出与 `m0/abi/*.json`。该任务不是 required CI；它的日志只能记录哈希、版本和
探针结果，不分发厂商输入文件。

### 3.3 compile-fail 与 auto-trait

compile-fail 测试至少固定以下不可编译行为：

- `Sdk` 仍被 `Camera` 借用时不能 shutdown；
- `Camera` 仍被 `Measurement` 或 `CallbackMeasurement` 独占借用时不能 close、再次 start
  或调用不属于该状态的方法；
- 借用输入构造的 `ImageRef` 不能逃逸输入生命周期；
- 文件传输独占借用期间不能重新借用相机；
- callback handler 必须满足 `Send + 'static`，不能捕获 `Rc` 或短期借用。

auto-trait 断言同时覆盖正向和负向：会话与设备 guard 保持 `!Send + !Sync`，拥有化帧、
图像和异常事件保持预期的 `Send + Sync`，不应复制的所有权类型保持 `!Clone`。公共类型的
这些变化视为 API 兼容性变化，必须通过评审而不是更新快照掩盖。

trybuild 的诊断文本只在 Ubuntu / Rust `1.97.0` 下作为基线。普通跨平台和 MSRV
`cargo test` 只编译该 harness 而不执行快照；固定 UI job 通过 `MV3DLP_RUN_UI=1` 显式启用，
避免编译器诊断措辞变化破坏与安全契约无关的日常测试。

### 3.4 Miri

Miri 只运行不触发 trybuild 子进程、不链接厂商库的 lib 单元测试：

```text
cargo +nightly-2026-07-09 miri test -p mv3d-lp-internal --lib --no-default-features --locked
cargo +nightly-2026-07-09 miri test -p mv3d-lp --lib --no-default-features --locked
```

Miri 可以发现 Rust 侧别名、越界、无效值和析构问题，但不会执行或证明闭源 DLL 的行为，
也不能验证厂商线程是否在 Rust 复制期间修改缓冲区。同名 C symbol stub 依赖普通系统链接器，
因此在 `cfg(miri)` 下不编译；raw status/out-param 门禁由跨平台普通单元测试负责，Miri 继续
执行纯 Rust 转换 helper、callback registry 和 fake Driver/Drop 测试。

## 4. required CI 矩阵

| 门禁 | runner/toolchain | 命令范围 | 厂商 SDK |
| --- | --- | --- | --- |
| 格式、Clippy、rustdoc | Ubuntu / stable | workspace 默认 feature，warnings deny | 不需要 |
| 单元、集成 | Ubuntu、Windows、macOS / stable | `cargo test --workspace --locked` | 不需要 |
| compile-fail | Ubuntu / 固定 Rust `1.97.0` | `MV3DLP_RUN_UI=1 cargo test --test compile_fail` | 不需要 |
| MSRV | Ubuntu / Rust `1.85.0` | workspace 默认 feature check/test | 不需要 |
| Miri | Ubuntu / `nightly-2026-07-09` | 两个 crate 的 `--lib --no-default-features` | 不需要 |

默认 feature 集为空，因此普通 `cargo test` 就是无 SDK 路径。required workflow 禁止使用
`--all-features`、`native` 或 `display-windows`。分支保护应把上述 job 设为 required；仓库中的
workflow 只能定义检查，不能替代托管平台上的分支保护设置。

## 5. 非 required 原生证据矩阵

| 任务 | 环境 | 目的 |
| --- | --- | --- |
| native build | 自管 Windows x64 MSVC + LPSDK 1.3.3.3 | 链接检查和无设备生命周期 smoke |
| display build | 同上 | Win32 feature 编译与纯包装测试 |
| x64 C++ ABI probe | 同上，厂商头文件与导入库哈希匹配 M0 | 检测头文件、编译器 ABI 或导出漂移 |
| x86 C++ ABI probe | 受控 32 位 MSVC 环境 | 维护历史审计快照，不扩大 Rust 支持范围 |
| 工业实机实验 | 记录设备、固件、运行时、网络和时长 | 提供环境观察，不能代替厂商保证 |

原生证据的解释规则见 `m5/native-contract-evidence.md`。任何实验通过都不得解除对未知
厂商并发、保留指针或稳定读窗口的措辞限制。

## 6. 版本、MSRV 与兼容性

- 首个候选版本为 `0.1.0`；当前不执行 registry package、publish dry-run 或正式发布。只有项目
  负责人以后明确恢复 registry 发布时，才进入对应演练和发布步骤。
- `0.1.x` 保持 MSRV `1.85`。提高 MSRV 至少提升 minor 版本并在 CHANGELOG 和发布说明中
  单独列出；补丁版本不得静默提高。
- 公开 API、auto-trait、默认 feature 或平台支持的破坏性变化在 `0.x` 阶段提升 minor；兼容
  修复提升 patch。
- internal 与 facade 始终同步版本。门面精确锁定 internal，防止实现边界在兼容范围解析中
  独立漂移。
- 每个 SDK 新版本建立新的文件哈希、ABI 和契约证据；运行时版本检查仍默认拒绝未审计版本。

## 7. 原生契约与发布判断

本项目当前无法取得 M2、M3、M4 所列问题的独立厂商书面保证。M5 把它明确记录为已知限制
和实现所依赖的原生假设，不把官方样例、配置文件、fake backend、Miri 或工业实机实验提升
为厂商保证。

M5 的发布决策是：无法取得厂商书面保证本身属于非阻塞已知假设。正式发布前必须在发布
说明中原样披露这些假设，并完成 `m5/native-contract-evidence.md` 的具名风险复核；未披露或
未复核会阻塞发布，但“没有厂商保证”这一事实本身不会。该复核表示项目接受精确审计环境中
的已知残余风险，不表示厂商确认，也不能扩大到其他 SDK、固件、设备或运行环境。若新证据
直接否定实现所需的稳定性假设，则必须先移除、禁用或重新设计受影响 API，之后才能发布。

## 8. M5 完成条件

M5 工程硬化完成要求：

- required CI 的格式、Clippy、跨平台测试、MSRV、compile-fail、auto-trait 和 Miri 全部通过；
- raw symbol fake 覆盖每个生产 FFI 调用点及失败 status/out-param 优先级，Driver fake 覆盖
  每个可达状态机操作，focused FFI 转换测试覆盖裸输出边界，关键 Drop 序列有精确日志断言；
- ABI 的 Rust 门禁通过，原生发布候选附带独立探针结果或明确记录未执行原因；
- 两个 crate 的许可证、README、默认 feature 与精确本地版本依赖通过检查；registry package
  和 fresh-consumer 演练随发布授权延期，manifest 继续使用 `publish = false`；
- 发布说明准确列出唯一原生目标、精确 SDK 版本、MSRV、feature 与残余原生假设；
- `m5/release-checklist.md` 逐项签署，未勾选项不得用口头结论替代。

工程硬化完成与 native 契约获得厂商保证是两个不同结论。发布产物和文档必须始终保留这一区分。
