//! 错误类型与 SDK 状态码。

use std::fmt;

use crate::sys;

/// 本 crate 的 `Result` 别名。
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// 本 crate 的错误。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// SDK 函数返回了非 `MV3D_LP_OK` 的状态码。
    #[error("{function} failed: {code}")]
    Sdk {
        /// 失败的 SDK 函数名。
        function: &'static str,
        /// SDK 返回的状态码。
        code: ErrorCode,
    },
    /// 本进程的 SDK 会话已经 `MV3D_LP_Finalize`；厂商约定每个进程只初始化一次，不能再次初始化。
    #[error("3DMVS SDK was finalized in this process and cannot be initialized again")]
    Finalized,
    /// 输入不满足 SDK 的内存约定，未调用 SDK。
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    /// 设备注册过 image callback；LPSDK 不能注销 callback，`CloseDevice` 前不能再用 pull 取图。
    #[error("pull grabbing is unavailable after an image callback was registered on this device")]
    ImageCallbackRegistered,
}

/// 把 SDK 返回值转换为 `Result`；通常经由 [`sdk_call!`] 调用。
pub(crate) fn check(function: &'static str, code: sys::MV3D_LP_STATUS) -> Result<()> {
    let code = code.cast_unsigned();
    if code == sys::MV3D_LP_OK {
        Ok(())
    } else {
        Err(Error::Sdk {
            function,
            code: ErrorCode::from_raw(code),
        })
    }
}

/// 调用返回状态码的 SDK 函数，失败时生成带函数名的 [`Error::Sdk`]。
///
/// 宏本身不含 `unsafe`，调用点仍须位于带 SAFETY 注释的 `unsafe` 块中。
macro_rules! sdk_call {
    ($function:ident($($argument:expr),* $(,)?)) => {
        $crate::error::check(stringify!($function), $crate::sys::$function($($argument),*))
    };
}
pub(crate) use sdk_call;

macro_rules! error_codes {
    ($($(#[$meta:meta])* $variant:ident = $code:ident,)+) => {
        /// `Mv3dLpDefine.h` 中的状态码；头文件未定义的值保存在 [`ErrorCode::Other`]。
        ///
        /// `Display` 输出头文件中的宏名与十六进制值，便于对照厂商文档。
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        #[non_exhaustive]
        pub enum ErrorCode {
            $($(#[$meta])* $variant,)+
            /// 头文件未定义的状态码。
            Other(u32),
        }

        impl ErrorCode {
            /// 由 SDK 返回值构造。
            pub const fn from_raw(code: u32) -> Self {
                match code {
                    $(sys::$code => Self::$variant,)+
                    other => Self::Other(other),
                }
            }

            /// 返回 SDK 状态码。
            pub const fn raw(self) -> u32 {
                match self {
                    $(Self::$variant => sys::$code,)+
                    Self::Other(code) => code,
                }
            }

            /// 头文件中的宏名。
            const fn name(self) -> Option<&'static str> {
                match self {
                    $(Self::$variant => Some(stringify!($code)),)+
                    Self::Other(_) => None,
                }
            }
        }
    };
}

error_codes! {
    /// 错误或无效的句柄。
    Handle = MV3D_LP_E_HANDLE,
    /// 不支持的功能。
    NotSupported = MV3D_LP_E_SUPPORT,
    /// 缓存已满。
    BufferOverflow = MV3D_LP_E_BUFOVER,
    /// 函数调用顺序错误。
    CallOrder = MV3D_LP_E_CALLORDER,
    /// 参数错误。
    Parameter = MV3D_LP_E_PARAMETER,
    /// 资源申请失败。
    Resource = MV3D_LP_E_RESOURCE,
    /// 无数据，例如取图超时。
    NoData = MV3D_LP_E_NODATA,
    /// 前置条件有误或运行环境已变化。
    Precondition = MV3D_LP_E_PRECONDITION,
    /// 版本不匹配。
    Version = MV3D_LP_E_VERSION,
    /// 传入的内存空间不足。
    NotEnoughBuffer = MV3D_LP_E_NOENOUGH_BUF,
    /// 异常图像。
    AbnormalImage = MV3D_LP_E_ABNORMAL_IMAGE,
    /// 动态库加载失败。
    LoadLibrary = MV3D_LP_E_LOAD_LIBRARY,
    /// 算法错误。
    Algorithm = MV3D_LP_E_ALGORITHM,
    /// 设备离线。
    DeviceOffline = MV3D_LP_E_DEVICE_OFFLINE,
    /// 无访问权限。
    AccessDenied = MV3D_LP_E_ACCESS_DENIED,
    /// 值超出范围。
    OutOfRange = MV3D_LP_E_OUTOFRANGE,
    /// 未知错误。
    Unknown = MV3D_LP_E_UNKNOW,
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(f, "{name} (0x{:08X})", self.raw()),
            None => write!(f, "unknown status code 0x{:08X}", self.raw()),
        }
    }
}
