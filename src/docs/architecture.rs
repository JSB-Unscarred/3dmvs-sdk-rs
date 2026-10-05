//! # 设计
//!
//! 本 crate 用所有权与借用表达 LPSDK 的调用约定：每种 native 资源由一个类型拥有，在 `Drop` 中释放；
//! 错误的调用顺序无法通过编译。
//!
//! ## 所有权
//!
//! | 类型 | 拥有的资源 | 释放时 |
//! | --- | --- | --- |
//! | [`Sdk`](crate::Sdk) | 一份会话引用 | 最后一份引用释放时 `MV3D_LP_Finalize` |
//! | [`Device`](crate::Device) | 设备 handle、callback 闭包、一份会话引用 | `MV3D_LP_CloseDevice`，再释放闭包 |
//! | [`Grabbing`](crate::Grabbing) | 主动取图的取流状态 | `MV3D_LP_StopMeasure` |
//! | [`CallbackGrabbing`](crate::CallbackGrabbing) | callback 取流状态 | `MV3D_LP_StopMeasure` |
//! | [`Image`](crate::Image) | 从 SDK buffer 复制出的数据 | 普通 Rust 内存 |
//!
//! ## 会话
//!
//! 一个进程只有一个 SDK 会话，[`Sdk`](crate::Sdk) 与每台 [`Device`](crate::Device) 各持有一份引用：
//!
//! - 第一次调用 [`Sdk::new`](crate::Sdk::new) 时初始化 SDK，失败后可以重试；
//! - 会话存活期间，`Sdk::new` 与 `clone` 得到同一会话；
//! - 最后一份引用释放时 SDK 反初始化。厂商要求每个进程只初始化一次，此后 `Sdk::new` 返回
//!   [`Error::Finalized`](crate::Error::Finalized)。
//!
//! 设备因此不借用 `Sdk`，可以放进结构体或移到其它线程。需要反复打开、关闭设备的程序（例如含多个测试的
//! 测试程序）应始终持有一份 `Sdk`，以免会话提前结束。
//!
//! ## 取流
//!
//! 取流守卫 [`Grabbing`](crate::Grabbing) 与 [`CallbackGrabbing`](crate::CallbackGrabbing) 独占设备，
//! 持有方式由 [`HoldsDevice`](crate::HoldsDevice) 决定：
//!
//! - `&mut Device`：借用设备，由 [`Device::start_grabbing`](crate::Device::start_grabbing) 与
//!   [`Device::start_grabbing_with`](crate::Device::start_grabbing_with) 创建，适合在一个作用域内取流；
//! - `Device`：按值持有，由 [`Grabbing::start`](crate::Grabbing::start) 与
//!   [`CallbackGrabbing::start`](crate::CallbackGrabbing::start) 创建，守卫可以存进结构体；开始失败或 `stop`
//!   时交还设备。
//!
//! `HoldsDevice` 是 sealed trait：守卫依赖每次借出的都是开始取流的同一台设备，任意 `BorrowMut` 实现
//! 无法保证这一点。两种方式下：
//!
//! - 取流期间不能再次开始取流、注册 callback 或关闭设备；
//! - 只有 [`Grabbing`](crate::Grabbing) 能主动取图；
//! - 设备关闭前取流一定已经停止。
//!
//! 参数读写、软触发与文件传输只需要 `&Device`，取流期间经守卫的 `Deref` 调用。文件传输阻塞到结束；
//! `Device` 不能在线程间共享，传输期间因此无法查询进度。
//!
//! ### Image callback 不能注销
//!
//! LPSDK 不提供注销 image callback 的接口：设备注册过 image callback 后，关闭前只能用 callback 取图。
//! 这一点无法用借用表达，改为运行时检查，此后 `start_grabbing` 返回
//! [`Error::ImageCallbackRegistered`](crate::Error::ImageCallbackRegistered)。
//!
//! `start_grabbing_with` 注册成功而 `MV3D_LP_StartMeasure` 失败时也是如此：调用返回错误并交还设备，但 callback
//! 已经注册，只能再次以 callback 方式重试。
//!
//! ## 清理失败
//!
//! `Drop` 忽略清理错误。需要检查时调用 [`Device::close`](crate::Device::close)、
//! [`Grabbing::stop`](crate::Grabbing::stop) 或 [`CallbackGrabbing::stop`](crate::CallbackGrabbing::stop)。
//!
//! `MV3D_LP_CloseDevice` 失败后 SDK 可能仍会回调，闭包与一份会话引用因此被泄漏，本进程不再反初始化 SDK。
//!
//! ## Callback
//!
//! SDK 在自己的线程中调用 callback，闭包必须是 `Fn + Send + Sync + 'static`。
//!
//! - image callback 收到的 [`Image`](crate::Image) 已经复制出来，可以直接发送到其它线程；
//! - [`ExceptionInfo`](crate::ExceptionInfo) 只在本次回调中有效；
//! - 闭包保留到设备关闭，重复注册会累积闭包；
//! - callback 中的 panic 会终止进程；
//! - 不要在 callback 中关闭设备或停止取流，SDK 可能阻塞或报错；应通过 channel 交给持有设备的线程。
//!
//! ## 图像
//!
//! 取流与图像处理的输出都复制为 [`Image`](crate::Image)。图像处理接口在 [`Sdk`](crate::Sdk) 上，同一会话的
//! 处理调用串行执行。
//!
//! SDK 按宽高与格式读取输入数据，长度不足会越界读，所以本 crate 在调用前检查输入：
//!
//! - 已知的非压缩格式：数据长度必须与宽高、[`ImageType::bits_per_pixel`](crate::ImageType::bits_per_pixel)
//!   严格对应；
//! - JPEG：数据非空即可；
//! - 其它格式：返回 [`Error::InvalidInput`](crate::Error::InvalidInput)。
//!
//! ## 线程
//!
//! - [`Sdk`](crate::Sdk) 与 [`DeviceInfo`](crate::DeviceInfo) 是 `Send + Sync`；
//! - [`Device`](crate::Device) 是 `Send` 但不是 `Sync`，同一台设备的调用由持有它的线程发起。
//!
//! ## 字符串
//!
//! 字符串参数使用 `&CStr`，例如 `c"ExposureTime"`。SDK 返回的字符串不保证是 UTF-8：设备信息以 `&CStr`
//! 借出，[`Parameter::String`](crate::Parameter::String) 中是 `CString`。
//!
//! ## 错误
//!
//! SDK 调用失败时返回 [`Error::Sdk`](crate::Error::Sdk)，其中有失败的 SDK 函数名与
//! [`ErrorCode`](crate::ErrorCode)；各状态码的含义以厂商文档为准。SDK 返回本 crate 不认识的取值时保留原始值，
//! 例如 [`Parameter::Other`](crate::Parameter::Other) 与 [`IpConfigMode::Other`](crate::IpConfigMode::Other)。
