//! # 设计原则
//!
//! 本 crate 把 LPSDK C API 的调用约定映射为 Rust 的所有权、借用与类型，而不是运行时状态检查。
//! 结构与 [realsense-rust](https://gitlab.com/tangram-vision/oss/realsense-rust) 一致，并与姊妹 crate
//! [`mvs-sdk`](https://crates.io/crates/mvs-sdk)（海康机器人 MVS 工业相机）保持对称：
//!
//! 1. `mv3d-lp-sys` 只包含审计过的声明、结构体布局断言与 `raw-dylib` 链接属性，只支持
//!    `x86_64-pc-windows-msvc`。
//! 2. 每种 native 资源对应一个拥有它的 Rust 类型，`Drop` 负责释放；方法直接调用 `sys`。
//! 3. 非法调用顺序在类型上不可表达。
//! 4. 优先使用 Rust 原生类型：`&CStr`、`CString`、`Option<Duration>`、`Ipv4Addr`、`Vec`。
//!
//! ## 所有权
//!
//! | 类型 | 拥有的资源 | 释放 |
//! | --- | --- | --- |
//! | [`Sdk`](crate::Sdk) | 一份 SDK 会话引用 | 最后一份引用释放时 `MV3D_LP_Finalize` |
//! | [`Device`](crate::Device) | handle、callback 闭包的强引用、一份会话引用 | `CloseDevice` → 释放闭包引用 |
//! | [`Grabbing`](crate::Grabbing) | pull 取流状态（可变借用设备） | `StopMeasure` |
//! | [`CallbackGrabbing`](crate::CallbackGrabbing) | callback 取流状态（可变借用设备） | `StopMeasure` |
//! | [`Image`](crate::Image) | 从 SDK buffer 复制的数据 | 普通 Rust 内存 |
//!
//! ## 会话
//!
//! 本进程只有一个 SDK 会话，由 [`Sdk`](crate::Sdk) 与每个 [`Device`](crate::Device) 以 `Arc` 共享，
//! 并以 `Weak` 登记在进程级 static 中：
//!
//! - 首次调用 [`Sdk::new`](crate::Sdk::new) 时 `MV3D_LP_Initialize`，成功后才登记，因此失败可以重试；
//! - 会话存活期间 `Sdk::new` 与 `clone` 得到同一会话；图像处理接口在 `Sdk` 上，工作线程可以 clone
//!   一份，丢掉 `Sdk` 后只要还有设备存活，仍能取回；
//! - 最后一份引用释放时 `MV3D_LP_Finalize`。厂商约定每个进程只初始化一次，之后 `Sdk::new` 返回
//!   [`Error::Finalized`](crate::Error::Finalized)。
//!
//! 设备不借用 `Sdk`，可以存入结构体或移到工作线程。同一测试二进制中的多个真机测试应共享一份
//! 保活的 `Sdk`，否则前一个测试释放会话后，后一个测试会得到 `Finalized`。
//!
//! ## 类型状态
//!
//! 取流守卫可变借用设备，借用检查器因此保证：
//!
//! - pull 取图只能在 [`Grabbing`](crate::Grabbing) 上调用；
//! - 取流期间不能再次开始取流、注册 callback 或关闭设备；
//! - 设备关闭前取流一定已经停止，`close` 不需要再判断取流状态。
//!
//! LPSDK 没有可靠的 callback 注销接口，注册 image callback 后该 handle 在 `CloseDevice` 前只按 callback
//! 模式使用。这一约束无法由借用表达，因此 [`Device::start_grabbing`](crate::Device::start_grabbing)
//! 在这种设备上返回 [`Error::ImageCallbackRegistered`](crate::Error::ImageCallbackRegistered)。
//!
//! 参数读写、软触发与文件传输只需要 `&Device`，取流期间经 `Deref` 仍可调用；文件传输阻塞到结束，
//! 因为 `Device` 不是 `Sync`，传输期间无法查询进度。守卫、设备与 `Sdk` 都标记了 `#[must_use]`。
//!
//! ## 失败与清理
//!
//! `Drop` 忽略清理错误；需要观察时调用 [`Device::close`](crate::Device::close)、
//! [`Grabbing::stop`](crate::Grabbing::stop) 或 [`CallbackGrabbing::stop`](crate::CallbackGrabbing::stop)。
//! `CloseDevice` 失败时 SDK 可能仍持有闭包指针，因此泄漏闭包的强引用与一份会话引用，闭包永不释放，
//! 本进程不再执行 Finalize。这一保证只依赖所有权与 `Arc` 计数。
//!
//! ## Callback
//!
//! 注册时把闭包放入 `Arc`，以 `Arc::into_raw` 的地址作为 `pUser`，trampoline 以具体类型 `F` 还原闭包，
//! 不经过锁或全局表。设备持有注册时的强引用，trampoline 在每次调用期间再持有一份：callback 中释放设备
//! 不会释放正在执行的闭包，最后一份引用可能在 SDK 线程释放。闭包必须是 `Fn + Send + Sync + 'static`，
//! 因为 SDK 在内部线程调用它。
//!
//! - LPSDK 不能注销 callback，闭包保留到 `CloseDevice`，重复注册会累积闭包；
//! - 参数一律按值传入：image callback 收到的 [`Image`](crate::Image) 已在回调返回前复制，
//!   [`ExceptionInfo`](crate::ExceptionInfo) 是只在本次回调期间有效的借用视图；
//! - panic 越过 `extern "C"` 时进程终止（Rust 1.81 起的语言行为）。
//!
//! 本 crate 假定 `CloseDevice` 成功返回后 SDK 不再以该 `pUser` 回调。在 callback 中关闭设备或结束取流时，
//! SDK 可能阻塞或返回错误，异常描述借用的 SDK 内存也只能依赖厂商保证；最后一个设备释放时还会在 SDK
//! 线程执行 Finalize。因此应通过 channel 通知 owner 线程。
//!
//! ## 图像
//!
//! 取流与图像处理的输出都在返回前复制为 [`Image`](crate::Image)。处理接口的输出 buffer 只在下一次
//! 处理调用前有效，因此同一会话的处理调用在锁内串行。作为处理输入时，SDK 按宽高与格式读取 buffer，
//! 本 crate 在调用前校验：已知的非压缩格式要求数据长度与宽高、[`ImageType::bits_per_pixel`](crate::ImageType::bits_per_pixel)
//! 严格对应，JPEG 只要求非空，其余格式一律返回 [`Error::InvalidInput`](crate::Error::InvalidInput)，
//! 避免 SDK 越界读。SDK 输出按厂商约定直接复制，不再二次校验。
//!
//! ## 线程
//!
//! [`Sdk`](crate::Sdk) 与 [`DeviceInfo`](crate::DeviceInfo) 是 `Send + Sync`；
//! [`Device`](crate::Device) 是 `Send` 但不是 `Sync`，同一 handle 的调用由 owner 串行发起，
//! 也保证 `GetImage` 的输出在复制前不被覆盖。
//!
//! ## 字符串
//!
//! 字符串参数使用 `&CStr`，不存在 interior NUL 错误。SDK 定长字符数组中的字符串以 NUL 结尾，
//! 厂商示例直接以 `%s` 读取：借用型 getter 返回截到 NUL 的 `&CStr`，缺少 NUL 的违约数据读作空串；
//! [`Parameter::String`](crate::Parameter::String) 这类拥有型输出在字段写满时保留整个字段。
//!
//! ## 错误
//!
//! SDK 失败统一为 [`Error::Sdk`](crate::Error::Sdk)，携带失败的函数名与
//! [`ErrorCode`](crate::ErrorCode)；各方法不再逐一列出可能的状态码，含义以厂商文档为准。
//! SDK 返回本 crate 无法解读的取值时保留原始值（如 [`Parameter::Other`](crate::Parameter::Other)、
//! [`IpConfigMode::Other`](crate::IpConfigMode::Other)），不伪造错误。错误信息为英文。
