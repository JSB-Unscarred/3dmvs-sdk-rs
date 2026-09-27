//! 进程级 SDK 会话：初始化、设备枚举、IP 配置与打开设备。

use std::ffi::CStr;
use std::fmt;
use std::net::Ipv4Addr;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use crate::error::sdk_call;
use crate::{Device, DeviceInfo, Error, IpConfig, Result, sys, write_ipv4};

/// 本进程的 SDK 会话登记。
///
/// `None` 表示尚未成功 `MV3D_LP_Initialize`；能 upgrade 表示会话存活；不能 upgrade 表示已经（或正在）
/// `MV3D_LP_Finalize`。只在 Initialize 成功后写入，因此失败可以重试；厂商约定每个进程只初始化一次，
/// Finalize 之后不再重新初始化。
static SESSION: Mutex<Option<Weak<Session>>> = Mutex::new(None);

/// 已初始化的 SDK 会话，由 [`Sdk`] 与每个 [`Device`] 通过 `Arc` 共享。
///
/// 最后一个持有者释放时调用 `MV3D_LP_Finalize`，因此设备不会比会话活得更久。
/// 设备的 `CloseDevice` 失败时会泄漏一份引用，使 Finalize 不再执行。
pub(crate) struct Session {
    /// 图像处理接口的输出buffer在下一次处理调用前有效，复制完成前需串行。
    processing: Mutex<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: strong 计数已归零：所有设备均已关闭，SESSION 中的 Weak 也不会再 upgrade 成功，
        // 之后没有 SDK 调用；Finalize 只在此处调用。
        unsafe { sys::MV3D_LP_Finalize() };
    }
}

/// 3DMVS SDK 的进程级入口。
///
/// 本进程只有一个会话，会话存活期间 [`Sdk::new`] 与 `clone` 得到的都是它。[`Device`] 持有会话引用
/// 而不借用 `Sdk`，因此可以存入结构体或移动到其它线程；`Sdk` 与全部设备都释放后 SDK 自动反初始化。
/// 以 `&self` 借用 `Sdk` 的方法保证调用时 SDK 已初始化。`Sdk` 是 `Send + Sync`。
#[derive(Clone)]
#[must_use = "the SDK is finalized once the last Sdk and device are dropped"]
pub struct Sdk {
    session: Arc<Session>,
}

impl Sdk {
    /// 返回本进程的 SDK 会话，首次调用时 `MV3D_LP_Initialize`。
    ///
    /// Initialize 失败返回 [`Error::Sdk`]，之后可以重试；会话 Finalize 之后返回 [`Error::Finalized`]。
    pub fn new() -> Result<Self> {
        let mut state = SESSION.lock().unwrap_or_else(PoisonError::into_inner);
        let session = match state.as_ref().map(Weak::upgrade) {
            Some(Some(session)) => session,
            Some(None) => return Err(Error::Finalized),
            None => {
                // SAFETY: 持锁且本进程尚未成功初始化，Initialize 不会并发或重复成功。
                unsafe { sdk_call!(MV3D_LP_Initialize()) }?;
                let session = Arc::new(Session {
                    processing: Mutex::new(()),
                });
                *state = Some(Arc::downgrade(&session));
                session
            }
        };
        Ok(Self { session })
    }

    /// SDK 版本字符串，无需先初始化。
    pub fn version() -> &'static CStr {
        // SAFETY: 厂商允许在 Initialize 之前调用，函数没有参数。
        let version = unsafe { sys::MV3D_LP_GetVersion() };
        if version.is_null() {
            return c"";
        }
        // SAFETY: 厂商约定返回指向静态存储、以 NUL 结尾的版本字符串。
        unsafe { CStr::from_ptr(version) }
    }

    /// 当前在线设备数量。
    pub fn device_count(&self) -> Result<u32> {
        let mut count = 0;
        // SAFETY: count 是可写输出。
        unsafe { sdk_call!(MV3D_LP_GetDeviceNumber(&raw mut count)) }?;
        Ok(count)
    }

    /// 枚举在线设备。
    ///
    /// 数量与列表分两次查询：期间下线的设备使条数少于计数，新上线的设备留到下次枚举。
    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        let count = self.device_count()?;
        // 与厂商示例一致，没有设备时不调用 GetDeviceList，避免传入悬垂指针与 0 容量。
        if count == 0 {
            return Ok(Vec::new());
        }
        let mut raw = vec![sys::MV3D_LP_DEVICE_INFO::default(); count as usize];
        let mut filled = 0;
        // SAFETY: raw 提供 count 个可写记录，SDK 最多写入 count 个；filled 是可写输出。
        unsafe {
            sdk_call!(MV3D_LP_GetDeviceList(
                raw.as_mut_ptr(),
                count,
                &raw mut filled
            ))
        }?;
        raw.truncate(filled as usize);
        Ok(raw.iter().map(DeviceInfo::from_raw).collect())
    }

    /// 按序列号写入设备的 IP 配置。
    pub fn set_ip_config(&self, serial_number: &CStr, config: IpConfig) -> Result<()> {
        let mut raw = config.to_raw();
        // SAFETY: serial_number 以 NUL 结尾，raw 是完整初始化的结构体。
        unsafe { sdk_call!(MV3D_LP_SetIpConfig(serial_number.as_ptr(), &raw mut raw)) }
    }

    /// 按 IP 打开设备。
    pub fn open_by_ip(&self, ip: Ipv4Addr) -> Result<Device> {
        let mut text = [0; 16];
        write_ipv4(&mut text, ip);
        Device::open(
            Arc::clone(&self.session),
            "MV3D_LP_OpenDeviceByIP",
            |handle| {
                // SAFETY: handle 是可写输出，text 以 NUL 结尾。
                unsafe { sys::MV3D_LP_OpenDeviceByIP(handle, text.as_ptr()) }
            },
        )
    }

    /// 按序列号打开设备。
    pub fn open_by_serial(&self, serial_number: &CStr) -> Result<Device> {
        Device::open(
            Arc::clone(&self.session),
            "MV3D_LP_OpenDeviceBySN",
            |handle| {
                // SAFETY: handle 是可写输出，serial_number 以 NUL 结尾。
                unsafe { sys::MV3D_LP_OpenDeviceBySN(handle, serial_number.as_ptr()) }
            },
        )
    }

    /// 图像处理接口的串行锁。
    pub(crate) fn lock_processing(&self) -> MutexGuard<'_, ()> {
        self.session
            .processing
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl fmt::Debug for Sdk {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sdk").finish_non_exhaustive()
    }
}
