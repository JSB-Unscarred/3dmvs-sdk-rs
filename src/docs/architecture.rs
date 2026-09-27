//! # 设计原则
//!
//! 本 crate 把 LPSDK C API 的调用约定映射为 Rust 的所有权、借用与类型，而不是运行时状态检查。
//! 结构与 [realsense-rust](https://gitlab.com/tangram-vision/oss/realsense-rust) 以及同一作者的
//! `mvs-sdk-rs` 一致：
//!
//! 1. `mv3d-lp-sys` 只包含审计过的声明、结构体布局断言和链接配置。
//! 2. 每种 native 资源对应一个拥有它的 Rust 类型，`Drop` 负责释放；方法直接调用 `sys`。
//! 3. 非法调用顺序在类型上不可表达。
//! 4. 优先使用 Rust 原生类型：`&CStr`、`CString`、`Option<Duration>`、`Ipv4Addr`、`Vec`。
//!
//! ## 所有权
//!
//! | 类型 | 拥有的资源 | 释放 |
//! | --- | --- | --- |
//! | [`Sdk`](crate::Sdk) | 一份 SDK 会话引用 | 最后一份引用释放时 `MV3D_LP_Finalize` |
//! | [`Device`](crate::Device) | handle、callback 闭包、一份会话引用 | `CloseDevice` → 释放闭包 |
//! | [`Grabbing`](crate::Grabbing) | pull 采集状态（可变借用设备） | `StopMeasure` |
//! | [`CallbackGrabbing`](crate::CallbackGrabbing) | callback 采集状态（可变借用设备） | `StopMeasure` |
//! | [`Image`](crate::Image) | 从 SDK 缓冲区复制的像素 | 普通 Rust 内存 |
//!
//! 会话用 `Arc` 共享，[`Device`](crate::Device) 不借用 [`Sdk`](crate::Sdk)，可以存入结构体或移到
//! 工作线程；只要还有设备存活，Finalize 就不会执行。厂商约定每个进程只初始化一次，
//! 会话存活期间 [`Sdk::new`](crate::Sdk::new) 返回同一会话，Finalize 之后返回
//! [`Error::Finalized`](crate::Error::Finalized)。
//!
//! ## 类型状态
//!
//! 采集守卫可变借用设备，借用检查器因此保证：
//!
//! - pull 取图只能在 [`Grabbing`](crate::Grabbing) 上调用；
//! - 采集期间不能再次开始采集、注册 callback 或关闭设备；
//! - 设备关闭前采集一定已经停止，`close` 不需要再判断采集状态。
//!
//! LPSDK 没有可靠的 callback 注销接口，注册 image callback 后该 handle 在 Close 前只按 callback
//! 模式使用。这一约束无法由借用表达，因此 [`Device::start_grabbing`](crate::Device::start_grabbing)
//! 在这种设备上返回
//! [`Error::ImageCallbackRegistered`](crate::Error::ImageCallbackRegistered)。
//!
//! 参数读写、软触发与文件传输只需要 `&Device`，采集期间经 `Deref` 仍可调用。
//!
//! ## 失败与清理
//!
//! `Drop` 忽略清理错误；需要观察时调用 [`Device::close`](crate::Device::close)、
//! [`Grabbing::stop`](crate::Grabbing::stop) 或 [`CallbackGrabbing::stop`](crate::CallbackGrabbing::stop)。
//! `CloseDevice` 失败时 SDK 可能仍持有闭包指针，因此泄漏闭包与一份会话引用，本进程不再执行
//! Finalize。这一保证只依赖所有权，不需要计数器或全局状态。
//!
//! ## Callback
//!
//! 注册时把 `Box<F>` 的地址作为 `pUser`，trampoline 以具体类型 `F` 还原闭包，不经过锁或全局表。
//! 闭包必须是 `Fn + Send + Sync + 'static`，因为 SDK 在内部线程调用它。
//!
//! - LPSDK 不能注销 callback，闭包保留到 Close，重复注册会累积闭包；
//! - image callback 收到的 [`Image`](crate::Image) 已在回调返回前复制；
//! - panic 越过 `extern "system"` 时进程终止（Rust 1.81 起的语言行为）；
//! - 不要在 callback 中关闭设备或改变采集状态，应通过 channel 通知 owner 线程。
//!
//! ## 图像
//!
//! 采集与图像处理的输出都在返回前复制为 [`Image`](crate::Image)。处理接口的输出缓冲区只在下一次
//! 处理调用前有效，因此同一会话的处理调用在锁内串行。作为处理输入时，SDK 按宽高与格式读取
//! 缓冲区，本 crate 在调用前校验长度，不一致时返回
//! [`Error::InvalidInput`](crate::Error::InvalidInput)。SDK 输出按厂商约定直接复制，不再二次校验。
//!
//! ## 线程
//!
//! [`Sdk`](crate::Sdk) 与 [`DeviceInfo`](crate::DeviceInfo) 是 `Send + Sync`；
//! [`Device`](crate::Device) 是 `Send` 但不是 `Sync`，同一 handle 的调用由 owner 串行发起。
//!
//! ## 错误
//!
//! SDK 失败统一为 [`Error::Sdk`](crate::Error::Sdk)，携带失败的函数名与
//! [`ErrorCode`](crate::ErrorCode)；各方法不再逐一列出可能的状态码，含义以厂商文档为准。
//! 字符串参数使用 `&CStr`，不存在 interior NUL 错误。
