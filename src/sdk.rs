//! 进程级 SDK 会话：初始化、设备枚举、IP 配置与打开设备。

use std::ffi::CStr;
use std::net::Ipv4Addr;
use std::ptr::{self, NonNull};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use crate::device_info::write_ipv4;
use crate::error::{check, sdk_call};
use crate::{Device, DeviceInfo, Error, ErrorCode, IpConfiguration, Result, sys};

/// 本进程是否已尝试 `MV3D_LP_Initialize`；厂商约定每个进程只初始化一次。
static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// 已初始化的 SDK 会话，由 [`Sdk`] 与每个 [`Device`] 通过 `Arc` 共享。
///
/// 最后一个持有者释放时调用 `MV3D_LP_Finalize`。设备 Close 失败时会泄漏一份引用，
/// 使 Finalize 不再执行。
pub(crate) struct Session {
    /// 图像处理接口的输出缓冲区在下一次处理调用前有效，复制完成前需串行。
    processing: Mutex<()>,
}

impl Drop for Session {
    fn drop(&mut self) {
        // SAFETY: 所有设备都持有会话引用，走到这里说明它们均已关闭；Finalize 只在此处调用。
        unsafe { sys::MV3D_LP_Finalize() };
    }
}

/// 3DMVS SDK 的进程级入口。
///
/// [`Device`] 持有同一会话的引用而不借用 `Sdk`，因此可以存入结构体或移动到其它线程；
/// `Sdk` 与全部设备都释放后 SDK 自动反初始化。`Sdk` 是 `Send + Sync`。
pub struct Sdk {
    session: Arc<Session>,
}

impl Sdk {
    /// 初始化 SDK。每个进程只能成功调用一次，之后返回 [`Error::AlreadyInitialized`]。
    pub fn initialize() -> Result<Self> {
        if INITIALIZED.swap(true, Ordering::AcqRel) {
            return Err(Error::AlreadyInitialized);
        }
        // SAFETY: 上面的原子标记保证本进程只调用一次。
        unsafe { sdk_call!(MV3D_LP_Initialize()) }?;
        Ok(Self { session: Arc::new(Session { processing: Mutex::new(()) }) })
    }

    /// SDK 版本字符串，无需先初始化。
    pub fn version() -> &'static CStr {
        // SAFETY: 厂商约定返回指向静态存储、以 NUL 结尾的版本字符串。
        unsafe {
            let version = sys::MV3D_LP_GetVersion();
            if version.is_null() { c"" } else { CStr::from_ptr(version) }
        }
    }

    /// 当前在线设备数量。
    pub fn device_count(&self) -> Result<u32> {
        let mut count = 0;
        // SAFETY: count 是可写输出。
        unsafe { sdk_call!(MV3D_LP_GetDeviceNumber(&raw mut count)) }?;
        Ok(count)
    }

    /// 枚举在线设备；数量与列表分两次查询，返回条数可能少于先前的计数。
    pub fn devices(&self) -> Result<Vec<DeviceInfo>> {
        let count = self.device_count()?;
        let mut raw = vec![sys::MV3D_LP_DEVICE_INFO::default(); count as usize];
        let mut filled = 0;
        // SAFETY: raw 提供 count 个可写记录，filled 是可写输出。
        unsafe { sdk_call!(MV3D_LP_GetDeviceList(raw.as_mut_ptr(), count, &raw mut filled)) }?;
        raw.truncate(filled as usize);
        Ok(raw.iter().map(DeviceInfo::from_raw).collect())
    }

    /// 按序列号写入设备的 IP 配置。
    #[allow(clippy::unused_self, reason = "借用 Sdk 保证调用时会话仍处于初始化状态")]
    pub fn set_ip_config(&self, serial_number: &CStr, config: IpConfiguration) -> Result<()> {
        let mut raw = config.to_raw();
        // SAFETY: serial_number 以 NUL 结尾，raw 是完整初始化的结构体。
        unsafe { sdk_call!(MV3D_LP_SetIpConfig(serial_number.as_ptr(), &raw mut raw)) }
    }

    /// 按 IP 打开设备。
    pub fn open_by_ip(&self, ip: Ipv4Addr) -> Result<Device> {
        let mut text = [0; 16];
        write_ipv4(&mut text, ip);
        self.open("MV3D_LP_OpenDeviceByIP", |handle| {
            // SAFETY: handle 是可写输出，text 以 NUL 结尾。
            unsafe { sys::MV3D_LP_OpenDeviceByIP(handle, text.as_ptr()) }
        })
    }

    /// 按序列号打开设备。
    pub fn open_by_serial(&self, serial_number: &CStr) -> Result<Device> {
        self.open("MV3D_LP_OpenDeviceBySN", |handle| {
            // SAFETY: handle 是可写输出，serial_number 以 NUL 结尾。
            unsafe { sys::MV3D_LP_OpenDeviceBySN(handle, serial_number.as_ptr()) }
        })
    }

    /// 图像处理接口的串行锁。
    pub(crate) fn lock_processing(&self) -> MutexGuard<'_, ()> {
        self.session.processing.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// 两个打开接口共用：只有状态成功且 handle 非空时才构造 [`Device`]。
    fn open(
        &self,
        function: &'static str,
        open: impl FnOnce(*mut sys::HANDLE) -> sys::MV3D_LP_STATUS,
    ) -> Result<Device> {
        let mut handle = ptr::null_mut();
        check(function, open(&raw mut handle))?;
        let handle =
            NonNull::new(handle).ok_or(Error::Sdk { function, code: ErrorCode::Handle })?;
        Ok(Device::new(handle, Arc::clone(&self.session)))
    }
}
