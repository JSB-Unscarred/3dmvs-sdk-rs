# 3dmvs-sdk-rs

海康威视 3D MVS 激光轮廓传感器 SDK 的 Rust 包装。项目目标是把 FFI、裸指针、SDK 全局状态、回调和厂商缓冲区等不安全边界隔离在内部，对外只提供安全 Rust API。

## 当前状态

M0：契约与 ABI 基线已经完成。

- 基线说明：docs/m0-contract-and-abi.md
- 机器可读清单：m0/sdk-baseline.json
- 35 个公开 API 的契约分类：m0/api-contracts.json
- x86/x64 ABI 快照：m0/abi
- 可重复校验脚本：tools/m0/verify.ps1
- C++ ABI 探针：tools/m0/abi_probe.cpp

仓库不包含厂商头文件、导入库、DLL 或安装包。校验和后续构建默认使用本机 3DMVS 安装目录。

## 验证 M0

在安装了 3DMVS 和 Visual Studio C++ 工具链的 Windows 主机上执行：

    powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\m0\verify.ps1 -Architecture all

脚本会核对 SDK 文件哈希与版本、公开头文件和 DLL 导出表，并分别编译、运行 x86 与 x64 ABI 探针。
