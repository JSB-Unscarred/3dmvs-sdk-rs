# M3 文件与图像处理契约

本文件记录 LPSDK 1.3.3.3 的文件传输、ImgProc 和 Windows 显示安全边界。

## FileTransfer

`MV3D_LP_FileAccessRead` 和 `MV3D_LP_FileAccessWrite` 启动异步传输。包装层在
`Camera` 中保存本地文件名与设备文件名的 `CString`，直到以下任一条件成立：

- `GetFileAccessProgress` 返回 `total > 0 && completed == total`；
- `CloseDevice` 确定成功。

提前丢弃 `FileTransfer` 不取消操作，也不释放文件名；可通过
`Camera::active_file_transfer` 恢复轮询。负进度、超过总量、进度倒退和 SDK 错误都不被
当作终态。首次得到非零总量后，后续总量必须保持不变；总量改变也不会释放文件名。启动失败的部分状态没有厂商说明，因此相机进入 `Faulted`，文件名保留到关闭。
关闭失败时有意泄漏两个小型 `CString`，避免后台 SDK 使用已经释放的地址。

当前接受、但未取得厂商书面确认的假设如下。实机实验只能用于发现反证和记录特定环境下的
观察，不能把这些假设补成厂商保证：

- 成功关闭设备会终止该句柄上的后台文件访问；
- `0/0` 表示尚未报告有效总量，而不是空文件完成；
- 进度相等且总量大于零表示完成。

文件名使用 SDK 原始窄字符串字节，不承诺 UTF-8、Windows ACP 或任意 Unicode 路径。

## ImgProc 输出所有权

安装的 1.3.3.3 DLL 和厂商示例表明四个输出型函数都返回 SDK 内部、可复用的输出
buffer。`ImageConvert` 即使收到调用方 buffer 也会替换 `pData`，且后续转换会复用同一
地址。因此包装层传入清零的输出描述符（转换时只额外设置目标类型），在同一次进程级
串行调用中完成：

1. 检查返回状态；
2. 校验输出类型、尺寸、指针和精确长度；
3. 执行有上限的 fallible allocation；
4. 立即复制到 `OwnedImage`；
5. 最后释放全局 SDK 调用锁。

SDK 指针不会跨越私有 Driver 边界。单个及聚合输入/输出上限为 512 MiB，所有尺寸计算
使用 checked arithmetic。两个多图算法的厂商上限均为 8 张，安全 API 要求 `1..=8`。

ImgProc、SaveImage 和 DisplayImage 的描述符没有 stride 字段，因此 M3 对已知未压缩格式
要求紧密打包且长度精确等于 `width × height × bytes_per_pixel`。M2 采集仍可保留更长的
厂商载荷；若帧含额外 padding，调用方必须先重打包后再进入 M3，避免 SDK 将 padding
误当成像素。JPEG 只要求非空并受聚合上限约束。

辅助数据按操作处理：

- 单图转换在尺寸不变时复制亮度和曝光时间戳；
- 普通深度转点云仅在输出尺寸仍等于输入尺寸时保留辅助数据；
- 环视点云输出丢弃第一张输入遗留的亮度和曝光指针；
- 深度拼接向 SDK 提供每张输入的亮度平面，并复制 SDK 返回的拼接亮度平面；不保留第一张输入遗留的曝光指针。

## 转换与文件格式

`ImageConvert` 只开放厂商头文件列出的六种转换：Depth→Mono8、Depth→RGB24、
Profile→PointCloud、Profile→ProfileABC32、ProfileABC32→PointCloud、
PointCloud→ProfileABC32。测试穷举全部 8×8 已知类型组合。

`SaveImage` 的 8 种已知图像类型与 11 种文件格式也采用显式允许矩阵，并由测试穷举
全部 8×11 组合。不支持的组合不会进入 FFI。

## DisplayImage

显示 API 只在 Windows 的 `display-windows` feature 下提供。公共层通过
`raw-window-handle` 借用 Win32 `HWND`，不接受裸指针或任意整数句柄。借用的窗口句柄和
图像数据至少保持到同步 SDK 调用返回；窗口的线程与绘制同步仍须遵守所用 GUI 框架。

支持 Mono8、Depth、RGB24、Profile、ProfileABC32 和 PointCloud。Mono8 不支持手动
范围；手动范围必须满足 `minimum < maximum`。

当前接受的保留假设是：头文件标为 `[IN]` 的图像载荷为只读，SDK 不会通过原始声明中的
可变指针写入 Rust 共享切片；图像和窗口句柄只在调用期间借用，SDK 不在 `DisplayImage`
返回后继续保留这些地址。相同的 `[IN]` 只读假设也适用于其他 ImgProc 和 SaveImage 输入。

## M5 残余假设决定

本项目当前无法取得文件传输终止语义、ImgProc 输出稳定窗口以及 `[IN]` 输入不修改/不保留的
独立厂商书面保证。M5 将这些条件登记为精确 LPSDK `1.3.3.3` 审计环境下的项目假设，
而不是厂商承诺；缺少书面保证本身不阻塞 `0.1.0`，但发布说明必须链接
`m5/native-contract-evidence.md` 并完成具名风险复核。任何工业观察或新资料一旦与这些
条件冲突，相关安全 API 必须先停发、移除或重设计，不能通过放宽测试继续发布。
