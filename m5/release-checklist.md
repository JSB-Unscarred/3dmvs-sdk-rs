# M5 发布检查表

候选版本：`0.1.0`  
候选 commit：________________  
负责人：________________  
复核人：________________  
日期：________________

任何标为阻塞的项目未完成时，不执行正式 crates.io 发布。预发布演练也必须记录未完成项，
不得把 dry-run 成功等同于 native 契约已经获得厂商保证。

当前 registry 发布已延期，两个 crate 必须保持 `publish = false`。只有项目负责人以后明确
授权发布时，才进入 E/F 阶段并解除该保护。

## A. 契约与范围（阻塞）

- [ ] 候选版本只声明 `x86_64-pc-windows-msvc` / LPSDK `1.3.3.3` 原生支持。
- [ ] `m0` 至 `m4` 契约与当前代码一致，所有新增 FFI 操作均已进入契约和测试账本。
- [ ] 已复核 `m5/native-contract-evidence.md` 的每个残余假设。
- [ ] 无法取得厂商保证的条目已登记为非阻塞已知假设，并完成具名风险接受与独立复核。
- [ ] 发布说明没有把官方样例、环境观察、工业实验、fake backend 或 Miri 写成厂商保证。
- [ ] 没有新证据直接否定实现所依赖的稳定窗口、只读输入或生命周期假设；若有则已阻止发布。
- [ ] 新 SDK/固件/文件哈希没有被旧证据默认为兼容。
- [ ] 仓库和发布 artifact 不含厂商头文件、LIB、DLL、配置、安装包或其他无再分发权文件。

风险决定：________________  
负责人签署：________________  
独立复核：________________

## B. 版本、元数据与 API（阻塞）

- [ ] `mv3d-lp` 与 `mv3d-lp-internal` 版本完全相同。
- [ ] 门面对 internal 使用精确 registry 版本 `=0.1.0`，同时保留开发用 path。
- [ ] internal crate 名称和两个 crate 的 crates.io owner 已确认。
- [ ] 在没有明确发布授权时，两个 manifest 仍保持 `publish = false`。
- [ ] 两个包包含 MIT LICENSE、README、repository、documentation 和 `rust-version = 1.85`。
- [ ] 默认 feature 集为空；`native` 与 `display-windows` 仍需显式启用。
- [ ] docs.rs 配置使用 no-default-features，不尝试取得厂商 SDK。
- [ ] 从上一个 tag 运行公共 API/semver 差异检查；首发则保存 `0.1.0` API 基线。
- [ ] auto-trait、公开错误、默认 feature 或 MSRV 的变化已在 CHANGELOG 单独列出。
- [ ] README 示例、Cargo feature 和代码中的 target/runtime 检查一致。

## C. required 无 SDK 门禁（阻塞）

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo check --workspace --locked`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`
- [ ] Ubuntu、Windows、macOS：`cargo test --workspace --locked`
- [ ] Rust 1.85.0：`cargo check --workspace --all-targets --locked`
- [ ] Rust 1.85.0：`cargo test --workspace --locked`
- [ ] Rust 1.97.0 / Ubuntu：`MV3DLP_RUN_UI=1 cargo test -p mv3d-lp --locked --test compile_fail`
- [ ] compile-fail `.stderr` 经过人工复核，没有用宽泛模式接受无关错误。
- [ ] 正向与负向 auto-trait 断言通过。
- [ ] raw symbol fake 覆盖 28/28 个生产 FFI 调用点；status 返回调用在解析 out-param 前保留
      原始失败码，Open 只检查预先初始化的输出槽是否非空，Version 单独覆盖空指针和无终止符，
      且实际调用的 vendor symbol 与账本一致。
- [ ] Driver fake 逐操作故障账本无缺项，且新增 Driver 操作会使账本测试失败。
- [ ] Drop/显式清理顺序测试覆盖正常、每个清理失败点、callback 排空和 Finalize 跳过。
- [ ] `cargo +nightly-2026-07-09 miri test -p mv3d-lp-internal --lib --no-default-features --locked`
- [ ] `cargo +nightly-2026-07-09 miri test -p mv3d-lp --lib --no-default-features --locked`
- [ ] 托管平台分支保护把上述 workflow job 设为 required。

CI run URL：________________

## D. 原生与工业证据（独立、不得污染 required CI）

- [ ] 受控 Windows 主机的 SDK 版本和文件哈希与 `m0/sdk-baseline.json` 一致。
- [ ] `cargo check --workspace --features native --locked`
- [ ] `cargo test --workspace --features native --no-run --locked`
- [ ] `cargo test --workspace --features display-windows --locked`
- [ ] x64 C++ ABI 探针与 `m0/abi/x64.json` 逐字段一致。
- [ ] 若复验 x86，结果只更新审计快照，不宣称当前 Rust crate 支持 x86 native。
- [ ] 无设备 Initialize/GetDeviceNumber/Finalize smoke 通过。
- [ ] 实机实验记录完整环境、时长、计数、错误和清理日志。
- [ ] 原生日志/artifact 不含厂商受限文件或设备凭据。

未执行项及原因：________________

## E. package 演练（仅在明确发布授权后，阻塞）

- [ ] 工作树干净，`Cargo.lock` 已提交且命令全部使用 `--locked`。
- [ ] `cargo package -p mv3d-lp-internal --list` 内容经过人工检查。
- [ ] internal `.crate` 包含源码、README 和 MIT LICENSE，不含工作区无关文件或厂商工件。
- [ ] `cargo publish --dry-run -p mv3d-lp-internal --locked` 通过。
- [ ] 在隔离环境解包并对 internal 默认 feature 执行 check/test/doc。
- [ ] 已用本地 registry 完成双 crate 演练，或记录 facade dry-run 必须等待 internal 进入索引。
- [ ] internal 发布并可从 crates.io 索引解析后，`cargo package -p mv3d-lp --list` 经过检查。
- [ ] `cargo publish --dry-run -p mv3d-lp --locked` 通过。
- [ ] fresh consumer 仅从 registry 依赖 `mv3d-lp = "0.1.0"` 时默认构建和测试不需要 SDK。

package 日志/哈希：________________

## F. 正式发布（阻塞）

- [ ] 已取得项目负责人解除两个 manifest `publish = false` 的明确授权。
- [ ] 冻结 release commit；版本、CHANGELOG 和文档均指向同一 commit。
- [ ] 先发布 `mv3d-lp-internal`，等待 `=0.1.0` 可解析并完成 registry smoke。
- [ ] 再发布 `mv3d-lp`，确认依赖解析到精确 internal 版本。
- [ ] 创建不可移动的签名 tag `v0.1.0`。
- [ ] GitHub Release 列出 MSRV、feature、唯一 target、精确 SDK、安装要求和残余假设链接。
- [ ] crates.io 页面不暗示仓库分发厂商 SDK 或得到厂商背书。
- [ ] docs.rs 对两个 crate 的 no-default-features 文档构建成功。

internal 发布 URL：________________  
facade 发布 URL：________________  
tag/Release URL：________________

## G. 发布后与回滚

- [ ] 从全新目录再次运行 registry fresh-consumer smoke。
- [ ] 验证 `cargo tree` 中 internal 恰为同版本，无意外 feature 启用。
- [ ] 记录 docs.rs、crates.io、GitHub Release 和 CI 的最终 URL。
- [ ] 监控安全报告与原生契约反证。
- [ ] 若发现安全或 ABI 问题，立即停止后续发布、yank 问题版本并发布修复版本；不删除已发布
      crate、不覆写 tag、不用相同版本替换内容。
- [ ] 在 SECURITY 和 CHANGELOG 中记录影响、受影响版本、缓解和修复版本。

最终结论（发布/阻止）：________________  
负责人签署：________________  
复核人签署：________________
