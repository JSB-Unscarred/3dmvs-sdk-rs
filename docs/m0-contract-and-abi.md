# M0：契约与 ABI 基线

状态：完成  
基线日期：2026-07-11  
目标 SDK：3DMVS 3.1.3 / LPSDK 1.3.3

## 1. 范围与结论

本阶段只固定后续 Rust 包装必须遵守的原生契约和 ABI，不实现 Rust FFI 或安全 API。

已经完成：

- 核对安装目录、公开头文件、x86/x64 导入库和运行时依赖。
- 固定关键文件长度、SHA-256、文件版本和运行时 API 版本。
- 核对公开头文件中的 35 个函数都存在于 x86/x64 DLL 导出表。
- 为 35 个公开函数逐项记录原始签名、安全门面状态、所有权和约束。
- 使用厂商头文件和导入库分别编译并运行 x86、x64 C++ ABI 探针。
- 固定公开结构体、联合体、回调指针的 size、alignment 和关键字段 offset。
- 完成 Initialize → GetDeviceNumber → Finalize 的无设备生命周期冒烟测试。
- 记录取帧、图像处理、回调、线程安全和文件访问的已知契约及未知项。

M0 不把厂商文件复制进仓库，也不对未公开导出符号建立绑定。

## 2. SDK 身份与安装布局

| 项目 | 基线 |
| --- | --- |
| 3DMVS 客户端版本 | 3.1.3，文件版本 3.1.3.0 |
| ReleaseNote 声明的 LPSDK | 1.3.3 |
| MV3D_LP_GetVersion 返回值 | 1.3.3.3 |
| Mv3dLp.dll 文件版本 | 1.3.3.3 |
| MvCameraControl3D.dll 文件版本 | 4.3.2.2 |
| C 开发指南 | 1.3.2 |
| 开发目录 | C:\Program Files (x86)\3DMVS\Development |
| LPSDK 运行时 | C:\Program Files (x86)\Common Files\Mv3dLpSDK |
| MV3D 基础运行时 | C:\Program Files (x86)\Common Files\MV3D |

开发指南比实际 LPSDK 低一个补丁版本，因此 1.3.3 新增接口不能仅凭 1.3.2 文档推断语义。

桌面目录 C:\Users\Mason\Desktop\Includes 中的三个头文件与安装目录中的文件长度和 SHA-256 完全相同。机器可读的完整路径、哈希和版本记录见 m0/sdk-baseline.json。

## 3. API 与调用约定

公开头文件共声明 35 个函数：

- Mv3dLpApi.h：29 个。
- Mv3dLpImgProc.h：6 个。

x86 和 x64 的 Mv3dLp.dll 均有 66 个 PE 导出，其中 65 个名称以 MV3D_LP_ 开头。35 个公开函数全部存在；另有 30 个 MV3D_LP_ 导出没有出现在公开头文件中。绑定范围只包含公开头文件声明，未公开导出一律不使用。

调用约定基线：

- 公开函数使用 C ABI。
- 回调类型使用 Windows system ABI；在 x86 上即 __stdcall。
- win32 和 win64 下的 Mv3dLp.lib 都是 MSVC 导入库，不是静态 SDK。

## 4. ABI 基线

探针由 MSVC 19.51 编译，并链接对应架构的厂商导入库。编译显式使用 WIN32 宏和 /Gd，和厂商示例工程的预处理环境及默认 C 调用约定保持一致。探针还对 35 个函数和 4 个回调的完整 C++ 类型执行编译期断言。

完整值见：

- m0/abi/x86.json
- m0/abi/x64.json

容易出错的差异摘要：

| 类型 | x86 size/alignment | x64 size/alignment |
| --- | ---: | ---: |
| HANDLE | 4/4 | 8/8 |
| MV3D_LP_DEVICE_INFO | 268/4 | 268/4 |
| MV3D_LP_IMAGE_DATA | 88/8 | 112/8 |
| MV3D_LP_PARAM_INFO 联合体 | 264/8 | 264/8 |
| MV3D_LP_PARAM | 288/8 | 288/8 |
| MV3D_LP_FILE_ACCESS | 40/4 | 48/8 |
| MV3D_LP_FILE_ACCESS_PROGRESS | 48/8 | 48/8 |
| MV3D_LP_PROFILE_DATA | 80/8 | 80/8 |
| MV3D_LP_POINTCLOUD_DATA | 48/8 | 48/8 |
| 回调函数指针 | 4/4 | 8/8 |

Rust FFI 层必须复现这些布局；不能根据字段直觉手工压缩结构体，也不能假定 x86 结构体都按 4 字节对齐。

两个架构下的 6 个公开枚举底层类型均由 MSVC 判定为有符号 32 位整数。状态码和枚举的原始 32 位十六进制值已经写入快照，包含值为 0xFFFFFFFF 和最高位为 1 的判别值；Rust 层不能用默认 repr 的 Rust 枚举直接接收未知值。

## 5. 生命周期契约

2026-07-11 的 x86、x64 冒烟状态都通过。下表中的设备数是 x64 捕获时的环境观察值：

| 调用 | 结果 |
| --- | --- |
| MV3D_LP_GetVersion | 1.3.3.3 |
| MV3D_LP_Initialize | 0x00000000 |
| MV3D_LP_GetDeviceNumber | 0x00000000 |
| 当时发现的设备数 | 0 |
| MV3D_LP_Finalize | 0x00000000 |

设备数只是当时环境的观察值，不属于必须匹配的基线。

安全层把进程级运行时和设备句柄分成两个状态机：

    Runtime:
      Uninitialized --Initialize--> Initialized --Finalize--> Finalized

    Device:
      NoHandle --OpenDeviceByIP/SN--> Open
      Open --StartMeasure--> Measuring
      Measuring --StopMeasure--> Open
      Open --CloseDevice--> Closed

Finalize 后能否再次 Initialize 没有公开契约，首版不允许从 Finalized 回到 Initialized。

只有合法状态转换进入 FFI。Drop 只能进行最佳努力清理，显式 stop、close 和 shutdown 才能报告错误。

## 6. 设备发现与句柄输出

MV3D_LP_GetDeviceList 由调用方提供结构体数组，SDK 返回实际数量。安全层先读取设备数、按上限分配并清零数组，再调用列表接口；设备可能在两次调用间变化，因此必须验证返回数量不超过容量，并允许有界重试。

OpenDeviceByIP 和 OpenDeviceBySN 的句柄输出先初始化为空。只有 SDK 返回成功且句柄非空时才能构造 Camera；失败路径不能让未初始化句柄进入 Drop。CloseDevice 返回后无条件使 Rust 侧句柄失效，即使关闭本身报告错误，也不能再次把同一值当作有效句柄使用。

## 7. 缓冲区与所有权契约

### 7.1 MV3D_LP_GetImage

开发指南和官方示例共同支持以下事实：

- 调用方传入清零的 MV3D_LP_IMAGE_DATA，不预分配 pData。
- 示例没有释放返回的 pData、pIntensityData 或 pExposureTimeStamp。
- SDK 配置文件包含内部图像缓存节点。

据此，返回指针视为 SDK 拥有的借用缓冲区。文档没有给出它们精确失效于下一次取帧、任意 SDK 调用还是停止采集，因此安全层不得把借用暴露给调用者。

M1 的硬约束：

- FFI 返回后，在进行任何下一次 SDK 调用前完成长度校验和深拷贝。
- 数据、强度和曝光时间戳分别复制到 Rust 拥有的容器。
- 对长度计算使用 checked arithmetic，并验证空指针与长度组合。
- 第一版只公开拥有所有权的 Frame，不公开零拷贝视图。

图像格式基线：

| SDK 格式 | 元素解释 |
| --- | --- |
| Mono8 | u8 |
| Depth | 有符号 i16 |
| Profile ABC16 | 每点三个 i16 |
| Profile ABC32 | 每点三个 32 位整数 |
| PointCloud ABC32f | 每点三个 f32 |

M0 只支持 Windows x86/x64 MSVC 目标，两者都是小端。安全层先把 SDK 输出复制成字节，再用 from_le_bytes 一类的逐元素解码构造 i16、i32 或 f32；不能把厂商指针直接转换成对齐后的 Rust 数值切片。每种格式都必须验证数据长度能被元素大小或像素步长整除。

nTimeStamp 的单位为毫秒。pExposureTimeStamp 的元素单位为毫秒，元素数量等于 nHeight。

### 7.2 图像处理输出

官方 DepthToPointCloud 示例同样不给输出结构预分配或释放内存；SDK 配置还存在算法输出缓存。MapDepthToPointCloud、Round 和 Mosaic 的输出先按 SDK 内部缓冲区处理，返回后立即深拷贝，并在初版中全局串行化图像处理调用。

ImageConvert 缺少可验证的官方调用示例，暂不进入首个安全 API。

SaveImage 的文件名编码、调用是否同步完成写入以及输入指针的最短寿命没有明确契约。DisplayImage 还涉及外部窗口句柄的线程亲和性和寿命。两者都保留原始绑定，但暂不进入安全门面。

## 8. 阻塞、回调与并发

MV3D_LP_GetImage 的超时语义：

- 0：立即返回。
- 0xFFFFFFFF：无限等待。
- 其他值：毫秒。
- 超时由 SDK 内部等待并返回 NoData。

首版安全 API 只接受有限 Duration。由于文档没有可中断等待契约，无限等待不映射为安全接口。

Duration 到 SDK 毫秒值的转换规则固定为：

- 0 映射为立即返回的 0。
- 非零但不足 1 毫秒的值向上取整为 1。
- 最大有限值是 0xFFFFFFFE 毫秒。
- 超过最大值时返回范围错误，不截断也不饱和。
- 安全层在任何情况下都不生成 0xFFFFFFFF。

回调文档明确要求在 Start 前注册，并提示不要在回调中调用其他 SDK API。官方回调示例也会立即复制数据。下列关键事实仍未知：

- 回调是否并发进入。
- Stop 或 Close 返回时是否保证所有回调已经退出。
- 是否允许重复注册。
- 传入空回调是否等价于注销。
- 用户指针在停止、关闭后的精确寿命。

因此 M1 不公开回调模式，只实现同步有限超时取帧。以后若引入回调，桥接函数只能校验、复制和发送到有界队列，不能调用其他 SDK API；在没有退出保障前，不能释放传给 SDK 的回调上下文。

公开文档也没有承诺 API 线程安全。初始安全模型为：

- 所有 SDK 调用经进程级锁串行化。
- Camera 默认不实现 Send 和 Sync。
- 会改变设备状态的方法使用独占可变访问。
- 异步包装由单一工作线程独占 Camera，通过消息传递提供并发使用体验。

## 9. 参数、字符串和保留字段

MV3D_LP_PARAM 含有由 enParamType 判别的联合体。安全层读取联合体前必须验证判别值，并把未知判别值作为错误处理。

其他固定规则：

- MV3D_LP_STATUS 是有符号 32 位整数；错误码以原始位模式解释，未知状态码必须原样保留。
- MV3D_LP_MAX_STRING_LENGTH 为 256。
- MV3D_LP_MAX_ENUM_COUNT 为 16。
- nSupportedNum 必须先验证不大于 16。
- 固定字符数组按有界 NUL 字符串读取，不允许越界寻找终止符。
- 厂商没有声明设备文本字段的字符编码；底层保留原始字节，对外只能提供可失败或有损的文本转换，不能假定 UTF-8。
- 传入 C 的保留字段全部清零。
- SDK 返回的未知枚举值必须保留为 Unknown，不得构造无效 Rust 枚举。

## 10. 暂缓接口

LPSDK 1.3.3 新增文件导入导出接口，但本机 C 指南仍为 1.3.2，官方样例没有实际调用。同步或异步行为、路径指针寿命、进度查询、取消和关闭语义均未确认，因此文件访问接口暂不进入安全门面。

Mv3dLpApi.h 从 GetDeviceIP 到 RegisterBatchProfileCallBack 的整个尾部区块被厂商标记为“废弃接口”。这些函数仍纳入 ABI 和原始绑定完整性检查，但安全门面不公开它们。新接口使用 GetDeviceList 和 GetImage 取代对应功能。

ImgProc 中的 ImageConvert、SaveImage 和 DisplayImage 因上述所有权、编码或窗口线程契约缺口暂缓。

未公开的 30 个 MV3D_LP_ 导出同样不进入绑定。

## 11. 分发边界

安装目录只发现第三方开源软件声明，没有找到海康威视 SDK 头文件、导入库、DLL 或安装包的再分发授权。仓库当前策略是：

- 不提交厂商二进制、头文件或安装包。
- 构建和校验从本机安装目录发现 SDK。
- 发布生成后的 Rust 绑定前另行完成法律与许可证审查。

## 12. 复验

在仓库根目录运行：

    powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\m0\verify.ps1 -Architecture all

校验脚本会：

1. 验证基线文件的长度、SHA-256 与版本。
2. 从三个公开头文件提取函数名并核对 DLL 导出。
3. 分别建立 x86、x64 MSVC 环境。
4. 编译并运行 tools/m0/abi_probe.cpp。
5. 将实际 JSON 与 m0/abi 下的快照逐字段比较。

列入清单的 SDK、配置、样例或文档工件发生变化，或者已探测的 API、版本和 ABI 发生漂移时，脚本会失败；接受新版 SDK 时必须显式审查并更新基线。

## 13. M1 的进入条件

M1 可以开始，但必须遵循以下边界：

- 只绑定 35 个公开函数。
- FFI 回调使用 system ABI。
- 取帧和图像处理返回值立即深拷贝。
- 仅提供有限超时的同步取帧。
- SDK 调用默认全局串行化，Camera 默认不跨线程。
- 回调、废弃 API、ImageConvert、SaveImage、DisplayImage、文件访问和未公开导出保持封闭，直到补齐契约证据。

## 14. 证据索引

所有证据均来自本机 SDK 安装，只读核对：

- 公开声明：Development\Includes\Mv3dLpApi.h 和 Mv3dLpImgProc.h。
- ABI 类型定义：Development\Includes\Mv3dLpDefine.h。
- 版本差异：C:\Program Files (x86)\3DMVS\ReleaseNote.txt 第 16 至 21 行，以及 Development\Documentations 下的 V1.3.2 C 指南。
- 拉流输出由 SDK 提供：Development\Samples\C\SimpleView_FetchFrame\main.cpp 第 39 至 44 行。
- 多帧保留前主动复制：Development\Samples\VC\BasicDemo_DepthMosaicSingle\BasicDemoDlg.cpp 第 222 至 249 行，核心复制在第 239 至 243 行。
- 深度转点云输出由 SDK 提供：Development\Samples\C\SimpleView_DepthToPointCloud\main.cpp 第 46 至 60 行。
- 多输入拼接和环形转换：Development\Samples\VC\BasicDemo_DepthMosaic\BasicDemoDlg.cpp 第 226 至 260 行，以及 BasicDemo_DepthToPointCloudRound\BasicDemoDlg.cpp 第 232 至 264 行。
- 回调内立即复制：Development\Samples\VC\BasicDemo_Callback\BasicDemoDlg.cpp 第 823 至 908 行。
- 简单回调注册和收尾：Development\Samples\C\SimpleView_CallBack\main.cpp 第 13 至 24、53 至 64 行。
- 多设备线程收尾顺序：Development\Samples\VC\MultipleCamera\MyCamera.cpp 第 22 至 45、115 至 170、228 至 239 行。
- 内部图像与算法缓存：C:\Program Files (x86)\Common Files\Mv3dLpSDK\Runtime\Win64_x64\Mv3dLpCfg.ini 第 17 至 24、49 至 56、75 至 79 行。
- 文件接口只有绑定而无 C 样例：Development\DotNet\win64\Mv3dLpNet.XML 第 152 至 174 行，以及 Development\Samples\Python\Mv3dLpImport\Mv3dLpApi.py 第 297 至 339 行。

CHM 中用于核对的主题包括 MV3D_LP_GetImage、MV3D_LP_IMAGE_DATA 和回调取图流程。文档明确给出超时、时间戳单位、曝光时间戳数量以及回调中不建议调用其他 SDK API 的说明。

证据强度按以下方式解释：

| 级别 | 含义 | 本基线中的例子 |
| --- | --- | --- |
| 文档保证 | 厂商头文件或开发指南明确说明 | 超时取值、时间戳单位、废弃 API 标记 |
| 样例观察 | 官方样例展示了调用方式，但不等于完整寿命保证 | GetImage 不预分配输出、保留多帧前深拷贝 |
| 配置佐证 | 官方运行时配置揭示内部实现资源 | BufferNode 和 AlgorithmNode |
| 保守策略 | 文档未说明时，为内存安全采用的限制 | 立即深拷贝、全局串行、暂缓回调和文件接口 |
