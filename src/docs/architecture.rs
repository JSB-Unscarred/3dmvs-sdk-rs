//! # 设计
//!
//! 本 crate 用所有权与借用表达 LPSDK 的调用约定：每种 native 资源由一个 Rust 类型拥有并在 `Drop`
//! 中释放，错误的调用顺序在编译期就无法写出。
//!
//! ## 所有权
//!
//! | 类型 | 拥有的资源 | 释放时 |
//! | --- | --- | --- |
//! | [`Sdk`](crate::Sdk) | 一份 SDK 会话 | 最后一份会话释放时 `MV3D_LP_Finalize` |
//! | [`Device`](crate::Device) | 设备 handle、callback 闭包、一份会话 | `MV3D_LP_CloseDevice`，再释放闭包 |
//! | [`Grabbing`](crate::Grabbing) | 主动取图的取流状态 | `MV3D_LP_StopMeasure` |
//! | [`CallbackGrabbing`](crate::CallbackGrabbing) | callback 取流状态 | `MV3D_LP_StopMeasure` |
//! | [`Image`](crate::Image) | 从 SDK buffer 复制出的数据 | 普通 Rust 内存 |
//!
//! ## 会话
//!
//! 一个进程只有一个 SDK 会话，[`Sdk`](crate::Sdk) 与每个 [`Device`](crate::Device) 各持有一份：
//!
//! - 第一次调用 [`Sdk::new`](crate::Sdk::new) 时初始化 SDK，失败后可以重试；
//! - 会话存活期间，`Sdk::new` 与 `clone` 得到同一会话；图像处理接口在 `Sdk` 上，工作线程可以 `clone` 一份；
//! - `Sdk` 与所有设备都释放后 SDK 反初始化。厂商要求每个进程只初始化一次，之后 `Sdk::new` 返回
//!   [`Error::Finalized`](crate::Error::Finalized)。
//!
//! 设备不借用 `Sdk`，可以放进结构体或移到其它线程。程序若先后多次创建并释放全部 `Sdk` 与设备
//! （例如同一测试程序中的多个测试），应保留一份 `Sdk`，否则后面的调用会得到 `Finalized`。
//!
//! ## 取流
//!
//! [`Device::start_grabbing`](crate::Device::start_grabbing) 与
//! [`Device::start_grabbing_with`](crate::Device::start_grabbing_with) 返回的守卫可变借用设备，因此：
//!
//! - 只有 [`Grabbing`](crate::Grabbing) 能主动取图；
//! - 取流期间不能再次开始取流、注册 callback 或关闭设备；
//! - 设备关闭前取流一定已经停止。
//!
//! LPSDK 不能注销 image callback，注册后这台设备在关闭前只能用 callback 取图。这一点无法用借用表达，
//! 所以在这种设备上调用 `start_grabbing` 会返回
//! [`Error::ImageCallbackRegistered`](crate::Error::ImageCallbackRegistered)。`start_grabbing_with` 在注册成功、
//! `MV3D_LP_StartMeasure` 失败时也是如此：调用返回错误，但 callback 已经注册，只能再次以 callback 方式重试。
//!
//! 参数读写、软触发与文件传输只需要 `&Device`，取流期间经守卫的 `Deref` 仍可调用。文件传输会阻塞到
//! 结束，由于 `Device` 不能在线程间共享，传输期间无法查询进度。
//!
//! ## 清理失败
//!
//! `Drop` 忽略清理错误；需要检查时调用 [`Device::close`](crate::Device::close)、
//! [`Grabbing::stop`](crate::Grabbing::stop) 或 [`CallbackGrabbing::stop`](crate::CallbackGrabbing::stop)。
//! `MV3D_LP_CloseDevice` 失败时 SDK 可能仍会调用已注册的闭包，因此闭包与会话都会泄漏，本进程不再
//! 反初始化 SDK。
//!
//! ## Callback
//!
//! SDK 在自己的线程中调用 callback，所以闭包必须是 `Fn + Send + Sync + 'static`。
//!
//! - image callback 收到的 [`Image`](crate::Image) 已经复制出来，可以直接发送到其它线程；
//! - [`ExceptionInfo`](crate::ExceptionInfo) 只在本次回调中有效；
//! - 闭包保留到设备关闭，重复注册会累积闭包；
//! - callback 中的 panic 会终止进程；
//! - 不要在 callback 中关闭设备或停止取流，SDK 可能阻塞或报错。应通过 channel 交给持有设备的线程处理。
//!
//! ## 图像
//!
//! 取流与图像处理的输出都复制为 [`Image`](crate::Image)。作为处理输入时，SDK 按宽高与格式读取数据，
//! 所以本 crate 在调用前检查：已知的非压缩格式要求数据长度与宽高、
//! [`ImageType::bits_per_pixel`](crate::ImageType::bits_per_pixel) 严格对应，JPEG 只要求非空，
//! 其它格式返回 [`Error::InvalidInput`](crate::Error::InvalidInput)。同一会话的图像处理调用会串行执行。
//!
//! ## 线程
//!
//! [`Sdk`](crate::Sdk) 与 [`DeviceInfo`](crate::DeviceInfo) 是 `Send + Sync`；
//! [`Device`](crate::Device) 是 `Send` 但不是 `Sync`，同一台设备的调用需要由持有它的线程发起。
//!
//! ## 字符串
//!
//! 字符串参数使用 `&CStr`，例如 `c"ExposureTime"`。设备信息等 SDK 字符串以 `&CStr` 返回，
//! [`Parameter::String`](crate::Parameter::String) 中是拥有的 `CString`。
//!
//! ## 错误
//!
//! SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)，其中包含失败的 SDK 函数名与
//! [`ErrorCode`](crate::ErrorCode)，各状态码的含义以厂商文档为准。SDK 返回本 crate 不认识的取值时保留
//! 原始值，例如 [`Parameter::Other`](crate::Parameter::Other) 与 [`IpConfigMode::Other`](crate::IpConfigMode::Other)。
