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

/// 已初始化的 SDK 会话，由 [`Sdk`] 与每个 [`Device`] 共享，最后一份释放时调用 `MV3D_LP_Finalize`。
/// 设备的 `CloseDevice` 失败时会泄漏一份，使 Finalize 不再执行。
pub(crate) struct Session {
    /// 图像处理接口的输出 buffer 在下一次处理调用前有效，复制完成前需串行。
    processing: Mutex<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: strong 计数已归零：所有设备均已关闭，SESSION 中的 Weak 也不会再 upgrade 成功，
        // 之后没有 SDK 调用；Finalize 只在此处调用。
        unsafe { sys::MV3D_LP_Finalize() };
    }
}

/// 本进程的 3DMVS SDK 会话，也提供图像处理接口。
///
/// [`Sdk::new`] 与 `clone` 得到同一会话；`Sdk` 与所有 [`Device`] 都释放后 SDK 反初始化。
#[derive(Clone)]
#[must_use = "the SDK is finalized once the last Sdk and device are dropped"]
pub struct Sdk {
    session: Arc<Session>,
}

impl Sdk {
    /// 取得本进程的 SDK 会话，首次调用时执行 `MV3D_LP_Initialize`。
    ///
    /// 初始化失败时返回 [`Error::Sdk`]，可以重试；SDK 反初始化之后返回 [`Error::Finalized`]。
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

    /// SDK 版本字符串，无需初始化。
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
    /// 数量与列表分两次查询，期间上线的设备要到下次枚举才出现。
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
        // 借用 `Sdk` 只为保证调用时 SDK 已初始化。
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
