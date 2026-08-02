# LPSDK 1.3.3.3 原生线程契约发布验证

> 当前状态：**待发布前实机验证**。2026-08-01 已通过原生测试目标的编译与链接；本机未配置设备夹具，因此本记录还不是锁定版本硬件行为的通过证据。公开 `Send + !Sync` 契约已由官方多线程示例、唯一所有权模型和 Rust 测试支持；实机测试属于发布验证，不是 trait soundness 或启用门槛。

## 发布验证规则

只有同时满足以下条件，才可把状态改为“通过”：

- 测试目标为 `x86_64-pc-windows-msvc`，实际加载的 DLL 版本精确为 `1.3.3.3`。
- 下表的全部场景在同一次测试进程中通过；本次测试由初始化线程完成一次 `Runtime` 初始化与 `shutdown`/`Finalize` 周期。
- 设备型号、脱敏标识、执行日期、线程摘要和逐场景结果均已填写。
- 本地 scratch 已清理，未上传、覆盖或删除任何设备文件；未提交完整原始日志。

编译通过只能证明测试与导入库可链接，不能替代 DLL、设备和跨线程析构的运行验证。

## 固定命令

先确保 `Mv3dLp.dll` 及其运行时依赖可由 Windows loader 找到，然后在一个预先存在、绝对路径为 ASCII 且为空的专用 scratch 目录中运行：

```powershell
$env:MV3D_LP_TEST_SERIAL = "<serial-from-secure-local-config>"
$env:MV3D_LP_TEST_DEVICE_READ_FILE = "<small-read-only-device-fixture>"
$env:MV3D_LP_TEST_LOCAL_SCRATCH_DIR = "<empty-absolute-ascii-scratch-directory>"
cargo test -p mv3d-lp-internal --features native --test native_thread_contract --locked single_runtime_cross_thread_contract -- --ignored --exact --nocapture --test-threads=1
```

设备须在线、可正常输出图像，device fixture 须是允许重复下载的小型只读文件。测试只调用 FileAccess read；host 端只在唯一子目录中创建不覆盖的目标，并只删除本次测试登记的精确文件和该空子目录。

## 本次发布验证记录

| 字段 | 记录 |
| --- | --- |
| 状态 | 待发布前实机验证 |
| 执行日期与时区 | 待填写 |
| 设备型号 | 待填写 |
| 设备脱敏标识 | 待填写，例如 `***1234` |
| DLL version | 待填写，必须为 `1.3.3.3` |
| target | `x86_64-pc-windows-msvc` |
| 线程摘要 | 待填写：A 初始化/Finalize；B 执行各 scoped handoff |
| 测试命令退出码 | 待填写 |
| scratch 清理 | 待填写 |

| 场景 | 结果 | 备注 |
| --- | --- | --- |
| 1. `Device` 在 B 查询、拉取图像、停止，并分别显式 close/隐式 Drop | 待执行 | |
| 2. `Measurement` 在 B 显式 stop/Drop | 待执行 | |
| 3. `CallbackMeasurement` 在 B 显式 stop/Drop，收到真实 callback 并完成排空 | 待执行 | |
| 4. exception callback 注册后 handoff 到 B close | 待执行 | 不要求设备主动产生异常 |
| 5. `Device` 先 handoff 到 B，再由 B 启动、完成并关闭 FileAccess | 待执行 | 验证设备所有权 handoff 后启动传输 |
| 6. A 启动 FileAccess，把借用式 `FileTransfer` scoped handoff 到 B 完成，join 后由 A 复用并关闭 `Device` | 待执行 | 直接验证 `FileTransfer: Send + !Sync` 的原生路径 |
| 7. B 启动 FileAccess 后丢弃 guard，A 恢复轮询至完成并复用 `Device` | 待执行 | 验证 `active_file_transfer()` 恢复语义 |
| 8. `Device` handoff 到 B，FileAccess start 后立即丢弃 guard，再关闭/Drop `Device` | 待执行 | 不要求观察到 `Running` |
| 9. GetImage、callback 到达和 progress 轮询分别受 10/15/60 秒 deadline 约束 | 待执行 | 无法中断的单次 DLL 调用或 callback drain 另受五分钟进程 watchdog 约束 |

## 当前非实机证据

| 日期 | 命令 | 结果 |
| --- | --- | --- |
| 2026-08-01 | `cargo test -p mv3d-lp-internal --features native --test native_thread_contract --no-run --locked` | 通过（仅编译与链接） |

实机失败时保留上表的脱敏场景结果和错误状态即可；序列号、device fixture 名称、完整本地路径及完整原始日志不得写入仓库。

若五分钟 watchdog 因 DLL 调用或 callback drain 卡死而终止进程，Rust RAII 无法执行。此时应先在专用 scratch 中人工核对并删除本次留下的精确 `mv3d-lp-native-<pid>-<nonce>` 子目录，再重新创建空 scratch；不得用递归清空命令代替核对。
