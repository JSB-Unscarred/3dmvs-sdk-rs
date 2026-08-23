use std::error::Error as StdError;
use std::fmt;

use crate::bindings;

/// Identifies the SDK operation associated with an error.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum Operation {
    /// 读取 SDK 版本；不要求先 Initialize。
    GetVersion,
    /// 初始化进程级 SDK session。
    Initialize,
    /// 释放进程级 SDK session。
    Finalize,
    /// 查询在线设备数量。
    GetDeviceNumber,
    /// 枚举在线设备。
    GetDeviceList,
    /// 按 IP 打开设备。
    OpenDeviceByIp,
    /// 按序列号打开设备。
    OpenDeviceBySn,
    /// 关闭设备。
    CloseDevice,
    /// 改写设备的 IP 配置。
    SetIpConfig,
    /// 启动采集。
    StartMeasure,
    /// 停止采集。
    StopMeasure,
    /// 发送一次软触发。
    SoftTrigger,
    /// 清空设备侧数据缓存。
    ClearDataBuffer,
    /// 以 pull 方式取一帧。
    GetImage,
    /// 注册图像 callback。
    RegisterImageDataCallback,
    /// 注册异常 callback。
    RegisterExceptionCallback,
    /// 读取一个参数。
    GetParam,
    /// 写入一个参数。
    SetParam,
    /// 执行一个命令节点。
    Execute,
    /// 从设备下载文件。
    FileAccessRead,
    /// 向设备上传文件。
    FileAccessWrite,
    /// 查询文件传输进度。
    GetFileAccessProgress,
    /// 把深度图转换为点云。
    MapDepthToPointCloud,
    /// 把一组深度图转换为点云。
    MapDepthToPointCloudRound,
    /// 在图像格式之间转换。
    ImageConvert,
    /// 拼接多张深度图。
    DepthMosaic,
    /// 把图像存成文件。
    SaveImage,
    /// 把图像渲染到 Win32 窗口。
    DisplayImage,
}

impl Operation {
    /// 返回厂商头文件中的接口名，`Display` 也走这里。
    #[must_use]
    pub const fn sdk_name(self) -> &'static str {
        match self {
            Self::GetVersion => "MV3D_LP_GetVersion",
            Self::Initialize => "MV3D_LP_Initialize",
            Self::Finalize => "MV3D_LP_Finalize",
            Self::GetDeviceNumber => "MV3D_LP_GetDeviceNumber",
            Self::GetDeviceList => "MV3D_LP_GetDeviceList",
            Self::OpenDeviceByIp => "MV3D_LP_OpenDeviceByIP",
            Self::OpenDeviceBySn => "MV3D_LP_OpenDeviceBySN",
            Self::CloseDevice => "MV3D_LP_CloseDevice",
            Self::SetIpConfig => "MV3D_LP_SetIpConfig",
            Self::StartMeasure => "MV3D_LP_StartMeasure",
            Self::StopMeasure => "MV3D_LP_StopMeasure",
            Self::SoftTrigger => "MV3D_LP_SoftTrigger",
            Self::ClearDataBuffer => "MV3D_LP_ClearDataBuffer",
            Self::GetImage => "MV3D_LP_GetImage",
            Self::RegisterImageDataCallback => "MV3D_LP_RegisterImageDataCallBack",
            Self::RegisterExceptionCallback => "MV3D_LP_RegisterExceptionCallBack",
            Self::GetParam => "MV3D_LP_GetParam",
            Self::SetParam => "MV3D_LP_SetParam",
            Self::Execute => "MV3D_LP_Execute",
            Self::FileAccessRead => "MV3D_LP_FileAccessRead",
            Self::FileAccessWrite => "MV3D_LP_FileAccessWrite",
            Self::GetFileAccessProgress => "MV3D_LP_GetFileAccessProgress",
            Self::MapDepthToPointCloud => "MV3D_LP_MapDepthToPointCloud",
            Self::MapDepthToPointCloudRound => "MV3D_LP_MapDepthToPointCloudRound",
            Self::ImageConvert => "MV3D_LP_ImageConvert",
            Self::DepthMosaic => "MV3D_LP_DepthMosaic",
            Self::SaveImage => "MV3D_LP_SaveImage",
            Self::DisplayImage => "MV3D_LP_DisplayImage",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.sdk_name())
    }
}

/// A status returned by the SDK, stored as its exact 32-bit bit pattern.
///
/// This is deliberately a newtype rather than a Rust enum so that statuses
/// introduced by a newer runtime remain representable.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct StatusCode(u32);

macro_rules! status_codes {
    ($($constant:ident = $raw:expr => $name:literal),+ $(,)?) => {
        impl StatusCode {
            $(
                #[doc = concat!("厂商头文件中的 `", $name, "`。")]
                pub const $constant: Self = Self($raw.cast_unsigned());
            )+

            /// Returns the vendor header name; statuses from a newer runtime return `None`.
            #[must_use]
            pub const fn name(self) -> Option<&'static str> {
                $(if self.0 == Self::$constant.0 {
                    return Some($name);
                })+
                None
            }
        }
    };
}

// 位模式与名字都以 bindings 为唯一来源，厂商头文件升级时只改 bindings。
status_codes! {
    OK = 0_i32 => "MV3D_LP_OK",
    INVALID_HANDLE = bindings::MV3D_LP_E_HANDLE => "MV3D_LP_E_HANDLE",
    UNSUPPORTED = bindings::MV3D_LP_E_SUPPORT => "MV3D_LP_E_SUPPORT",
    BUFFER_OVERFLOW = bindings::MV3D_LP_E_BUFOVER => "MV3D_LP_E_BUFOVER",
    INVALID_CALL_ORDER = bindings::MV3D_LP_E_CALLORDER => "MV3D_LP_E_CALLORDER",
    INVALID_PARAMETER = bindings::MV3D_LP_E_PARAMETER => "MV3D_LP_E_PARAMETER",
    RESOURCE_ERROR = bindings::MV3D_LP_E_RESOURCE => "MV3D_LP_E_RESOURCE",
    NO_DATA = bindings::MV3D_LP_E_NODATA => "MV3D_LP_E_NODATA",
    PRECONDITION_FAILED = bindings::MV3D_LP_E_PRECONDITION => "MV3D_LP_E_PRECONDITION",
    VERSION_MISMATCH = bindings::MV3D_LP_E_VERSION => "MV3D_LP_E_VERSION",
    INSUFFICIENT_BUFFER = bindings::MV3D_LP_E_NOENOUGH_BUF => "MV3D_LP_E_NOENOUGH_BUF",
    ABNORMAL_IMAGE = bindings::MV3D_LP_E_ABNORMAL_IMAGE => "MV3D_LP_E_ABNORMAL_IMAGE",
    LOAD_LIBRARY_FAILED = bindings::MV3D_LP_E_LOAD_LIBRARY => "MV3D_LP_E_LOAD_LIBRARY",
    ALGORITHM_ERROR = bindings::MV3D_LP_E_ALGORITHM => "MV3D_LP_E_ALGORITHM",
    DEVICE_OFFLINE = bindings::MV3D_LP_E_DEVICE_OFFLINE => "MV3D_LP_E_DEVICE_OFFLINE",
    ACCESS_DENIED = bindings::MV3D_LP_E_ACCESS_DENIED => "MV3D_LP_E_ACCESS_DENIED",
    OUT_OF_RANGE = bindings::MV3D_LP_E_OUTOFRANGE => "MV3D_LP_E_OUTOFRANGE",
    UNKNOWN = bindings::MV3D_LP_E_UNKNOW => "MV3D_LP_E_UNKNOW",
}

impl StatusCode {
    /// 按位保留 SDK 返回的状态；未在头文件中的取值同样可表示。
    #[must_use]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw.cast_unsigned())
    }

    /// 按位构造，供已经持有无符号位模式的调用方使用。
    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    /// 还原成 SDK 头文件里的有符号状态码。
    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0.cast_signed()
    }

    /// 取出原始 32 位模式。
    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// 是否为 `MV3D_LP_OK`。
    #[must_use]
    pub const fn is_ok(self) -> bool {
        self.0 == Self::OK.0
    }
}

impl fmt::Debug for StatusCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "StatusCode({name}, 0x{:08X})", self.0),
            None => write!(formatter, "StatusCode(0x{:08X})", self.0),
        }
    }
}

impl fmt::Display for StatusCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "{name} (0x{:08X})", self.0),
            None => write!(formatter, "unknown SDK status 0x{:08X}", self.0),
        }
    }
}

/// An error reported directly by an SDK function.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SdkError {
    operation: Operation,
    status: StatusCode,
}

impl SdkError {
    /// 记录一次失败的接口调用及其状态码。
    #[must_use]
    pub const fn new(operation: Operation, status: StatusCode) -> Self {
        Self { operation, status }
    }

    /// 失败的那个 SDK 接口。
    #[must_use]
    pub const fn operation(self) -> Operation {
        self.operation
    }

    /// 该接口返回的状态码。
    #[must_use]
    pub const fn status(self) -> StatusCode {
        self.status
    }
}

impl fmt::Display for SdkError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} failed with {}", self.operation, self.status)
    }
}

impl StdError for SdkError {}

/// 调用方传入值不满足 SDK 约束的具体原因。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum InputViolation {
    /// 值为空。
    Empty,
    /// 值中含有 NUL 字节，无法作为 C 字符串传入。
    InteriorNul,
    /// 值超出 SDK 字段容量。
    TooLong {
        /// 该字段允许的最大字节数。
        max: usize,
        /// 实际字节数。
        actual: usize,
    },
    /// 多图接口的输入张数不在允许区间内。
    ImageCount {
        /// 允许的最少张数。
        minimum: usize,
        /// 允许的最多张数。
        maximum: usize,
        /// 实际张数。
        actual: usize,
    },
    /// 输入图像的宽高、类型与数据长度互相矛盾。
    InvalidImageLayout {
        /// 不一致的那个字段名。
        field: &'static str,
    },
    /// 窗口当前拿不到句柄。
    WindowHandleUnavailable,
    /// 窗口句柄无法表示为 SDK 需要的形式。
    WindowHandleNotSupported,
    /// 窗口不是 Win32 `HWND`。
    NonWin32Window,
}

impl fmt::Display for InputViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("the value is empty"),
            Self::InteriorNul => formatter.write_str("the value contains a NUL byte"),
            Self::TooLong { max, actual } => write!(
                formatter,
                "the value has {actual} bytes; at most {max} are allowed"
            ),
            Self::ImageCount {
                minimum,
                maximum,
                actual,
            } => write!(
                formatter,
                "the image count is {actual}; expected {minimum}..={maximum}"
            ),
            Self::InvalidImageLayout { field } => {
                write!(formatter, "the image has an invalid {field}")
            }
            Self::WindowHandleUnavailable => {
                formatter.write_str("the window handle is unavailable")
            }
            Self::WindowHandleNotSupported => {
                formatter.write_str("the window handle cannot be represented")
            }
            Self::NonWin32Window => formatter.write_str("the window does not expose a Win32 HWND"),
        }
    }
}

/// SDK 返回的数据不满足其自身文档约定的具体原因。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum ContractViolation {
    /// SDK 返回了空指针。
    NullPointer {
        /// 返回空指针的字段名。
        field: &'static str,
    },
    /// SDK 返回的指针为空，长度却非零。
    NullPointerWithLength {
        /// 字段名。
        field: &'static str,
        /// SDK 同时声明的长度。
        length: usize,
    },
    /// SDK 报告的元素数超过了该字段的固定容量。
    CountExceedsCapacity {
        /// 字段名。
        field: &'static str,
        /// SDK 报告的元素数。
        count: usize,
        /// 该字段的固定容量。
        capacity: usize,
    },
    /// union 的判别值不在已知取值内，无法确定活跃成员。
    UnknownDiscriminant {
        /// 字段名。
        field: &'static str,
        /// 无法解释的判别值。
        raw: u32,
    },
    /// 由 SDK 字段推算长度时发生溢出。
    LengthOverflow {
        /// 字段名。
        field: &'static str,
    },
    /// SDK 返回的长度与其自身声明的期望值不符。
    LengthMismatch {
        /// 字段名。
        field: &'static str,
        /// SDK 自身声明的长度。
        expected: usize,
        /// 实际长度。
        actual: usize,
    },
    /// SDK 输出超过本 crate 允许拷贝的上限。
    OutputTooLarge {
        /// 字段名。
        field: &'static str,
        /// 本 crate 允许拷贝的上限。
        limit: usize,
        /// SDK 实际输出的大小。
        actual: usize,
    },
    /// SDK 在该字段返回了本 crate 无法解释的取值。
    InvalidValue {
        /// 字段名。
        field: &'static str,
    },
}

impl fmt::Display for ContractViolation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NullPointer { field } => write!(formatter, "{field} is null"),
            Self::NullPointerWithLength { field, length } => {
                write!(formatter, "{field} is null but its length is {length}")
            }
            Self::CountExceedsCapacity {
                field,
                count,
                capacity,
            } => write!(
                formatter,
                "{field} count {count} exceeds capacity {capacity}"
            ),
            Self::UnknownDiscriminant { field, raw } => {
                write!(formatter, "{field} contains unknown value 0x{raw:08X}")
            }
            Self::LengthOverflow { field } => {
                write!(formatter, "the computed length for {field} overflowed")
            }
            Self::LengthMismatch {
                field,
                expected,
                actual,
            } => write!(
                formatter,
                "{field} has length {actual}; expected {expected} for this SDK result"
            ),
            Self::OutputTooLarge {
                field,
                limit,
                actual,
            } => write!(
                formatter,
                "{field} has {actual} bytes, exceeding the configured limit of {limit}"
            ),
            Self::InvalidValue { field } => {
                write!(formatter, "{field} contains an invalid SDK value")
            }
        }
    }
}

/// 本 crate 全部接口共用的错误类型。
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// 当前 target 未链接厂商 SDK。
    UnsupportedPlatform,
    /// SDK 接口直接返回了失败状态。
    Sdk(SdkError),
    /// 调用方传入的值不满足 SDK 约束，未发起 native 调用。
    InvalidInput {
        /// 出问题的参数名。
        field: &'static str,
        /// 具体不满足哪条约束。
        violation: InputViolation,
    },
    /// 当前生命周期状态不允许该操作。
    InvalidState {
        /// 被拒绝的操作。
        operation: Operation,
        /// 该操作要求的状态。
        expected: &'static str,
        /// 当前实际状态。
        actual: &'static str,
    },
    /// SDK 调用成功，但返回的数据违反其文档约定。
    ContractViolation {
        /// 返回该数据的操作。
        operation: Operation,
        /// 具体违反了哪条约定。
        violation: ContractViolation,
    },
    /// 设备清理时 Stop 与 Close 双双失败，两个错误都保留。
    DeviceCleanup {
        /// Stop 的失败原因。
        stop: Box<Self>,
        /// Close 的失败原因。
        close: Box<Self>,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedPlatform => formatter
                .write_str("native 3DMVS support is available only on x86_64-pc-windows-msvc"),
            Self::Sdk(error) => error.fmt(formatter),
            Self::InvalidInput { field, violation } => {
                write!(formatter, "invalid {field}: {violation}")
            }
            Self::InvalidState {
                operation,
                expected,
                actual,
            } => write!(
                formatter,
                "{operation} requires state {expected}, but the current state is {actual}"
            ),
            Self::ContractViolation {
                operation,
                violation,
            } => write!(
                formatter,
                "{operation} returned data that violates the SDK contract: {violation}"
            ),
            Self::DeviceCleanup { stop, close } => write!(
                formatter,
                "device cleanup failed while stopping ({stop}) and closing ({close})"
            ),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::Sdk(error) => Some(error),
            Self::DeviceCleanup { close, .. } => Some(close.as_ref()),
            _ => None,
        }
    }
}

impl From<SdkError> for Error {
    fn from(error: SdkError) -> Self {
        Self::Sdk(error)
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as StdError;

    use super::{Error, Operation, SdkError, StatusCode};

    // 验证已知与未知 status 都保留厂商位模式和调用上下文。
    #[test]
    fn status_preserves_bits_and_operation() {
        let known = StatusCode::from_raw(0x8006_000D_u32.cast_signed());
        assert_eq!(known, StatusCode::DEVICE_OFFLINE);
        assert_eq!(known.bits(), 0x8006_000D);

        let unknown = StatusCode::from_bits(0xDEAD_BEEF);
        let error = SdkError::new(Operation::GetParam, unknown);
        assert_eq!(error.operation(), Operation::GetParam);
        assert_eq!(error.status(), unknown);
    }

    // 验证双重清理失败保留两次调用信息，并以 terminal Close 作为 source。
    #[test]
    fn cleanup_preserves_stop_and_close_failures() {
        let stop = SdkError::new(Operation::StopMeasure, StatusCode::RESOURCE_ERROR);
        let close = SdkError::new(Operation::CloseDevice, StatusCode::INVALID_HANDLE);
        let error = Error::DeviceCleanup {
            stop: Box::new(stop.into()),
            close: Box::new(close.into()),
        };

        assert_eq!(
            error.to_string(),
            format!("device cleanup failed while stopping ({stop}) and closing ({close})")
        );
        assert_eq!(
            StdError::source(&error).map(ToString::to_string),
            Some(close.to_string())
        );
    }
}
