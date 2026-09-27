//! 已打开的设备：handle 所有权、参数、文件传输与 exception callback。

use std::ffi::{CStr, c_void};
use std::fmt;
use std::mem;
use std::ptr::{self, NonNull};
use std::sync::Arc;

use crate::callback::{exception_trampoline, into_user_data};
use crate::error::sdk_call;
use crate::sdk::Session;
use crate::{ExceptionInfo, Parameter, ParameterValue, Result, sys};

/// 已打开的激光轮廓传感器。
///
/// `Device` 独占 native handle，释放时调用 `MV3D_LP_CloseDevice`；需要观察清理错误时调用
/// [`Device::close`]。设备持有 SDK 会话的引用，不借用 [`Sdk`](crate::Sdk)。
/// `Device` 是 `Send` 但不是 `Sync`，同一 handle 上的调用由 owner 串行发起。
#[must_use = "the device is closed when dropped"]
pub struct Device {
    /// 只在 `release` 中被取走，存活的设备总是持有 handle。
    handle: Option<NonNull<c_void>>,
    /// 交给 SDK 的闭包；LPSDK 不能注销 callback，只在 `CloseDevice` 成功后释放。
    callbacks: Vec<Arc<dyn Send + Sync>>,
    /// 注册过 image callback 后，Close 前不能再用 pull 取图。
    image_callback_registered: bool,
    session: Arc<Session>,
}

// SAFETY: 厂商示例在工作线程中使用 handle。Device 独占 handle 且不是 Sync，调用不会并发。
unsafe impl Send for Device {}

impl Device {
    pub(crate) const fn new(handle: NonNull<c_void>, session: Arc<Session>) -> Self {
        Self {
            handle: Some(handle),
            callbacks: Vec::new(),
            image_callback_registered: false,
            session,
        }
    }

    /// native handle，供尚未封装的 SDK 接口使用。
    ///
    /// 通过它改变采集、callback 注册或 handle 生命周期会破坏本 crate 的约定。
    pub fn as_raw_handle(&self) -> *mut c_void {
        self.handle.map_or(ptr::null_mut(), NonNull::as_ptr)
    }

    /// 发送一次软触发；触发模式与调用顺序由 SDK 检查。
    pub fn soft_trigger(&self) -> Result<()> {
        // SAFETY: handle 在设备存活期间有效。
        unsafe { sdk_call!(MV3D_LP_SoftTrigger(self.as_raw_handle())) }
    }

    /// 清空设备已缓存的帧。
    pub fn clear_buffer(&self) -> Result<()> {
        // SAFETY: handle 在设备存活期间有效。
        unsafe { sdk_call!(MV3D_LP_ClearDataBuffer(self.as_raw_handle())) }
    }

    /// 读取参数。
    pub fn get_parameter(&self, key: &CStr) -> Result<Parameter> {
        let mut raw = sys::MV3D_LP_PARAM::default();
        // SAFETY: key 以 NUL 结尾，raw 是清零的可写输出。
        unsafe {
            sdk_call!(MV3D_LP_GetParam(
                self.as_raw_handle(),
                key.as_ptr(),
                &raw mut raw
            ))
        }?;
        Ok(Parameter::from_raw(&raw))
    }

    /// 写入参数。
    pub fn set_parameter(&self, key: &CStr, value: ParameterValue<'_>) -> Result<()> {
        let mut raw = value.to_raw()?;
        // SAFETY: key 以 NUL 结尾，raw 的类型字段与写入的 union 成员一致。
        unsafe {
            sdk_call!(MV3D_LP_SetParam(
                self.as_raw_handle(),
                key.as_ptr(),
                &raw mut raw
            ))
        }
    }

    /// 执行 Command 节点。
    pub fn execute_command(&self, key: &CStr) -> Result<()> {
        // SAFETY: key 以 NUL 结尾。
        unsafe { sdk_call!(MV3D_LP_Execute(self.as_raw_handle(), key.as_ptr())) }
    }

    /// 把设备文件下载到主机，传输结束后返回。
    ///
    /// 调用阻塞整个传输；`Device` 不是 `Sync`，传输期间无法查询进度，因此不封装
    /// `MV3D_LP_GetFileAccessProgress`。
    pub fn download_file(&self, device_file: &CStr, local_file: &CStr) -> Result<()> {
        let mut access = file_access(local_file, device_file);
        // SAFETY: 调用阻塞到传输结束，两个以 NUL 结尾的文件名在此期间一直有效。
        unsafe {
            sdk_call!(MV3D_LP_FileAccessRead(
                self.as_raw_handle(),
                &raw mut access
            ))
        }
    }

    /// 把主机文件上传到设备，传输结束后返回；阻塞语义同 [`Device::download_file`]。
    pub fn upload_file(&self, local_file: &CStr, device_file: &CStr) -> Result<()> {
        let mut access = file_access(local_file, device_file);
        // SAFETY: 调用阻塞到传输结束，两个以 NUL 结尾的文件名在此期间一直有效。
        unsafe {
            sdk_call!(MV3D_LP_FileAccessWrite(
                self.as_raw_handle(),
                &raw mut access
            ))
        }
    }

    /// 注册 exception callback，替换之前的注册。
    ///
    /// SDK 在内部线程调用 `callback`；闭包保留到 Close，重复注册会累积闭包。
    /// callback 内的 panic 会在 FFI 边界终止进程。
    pub fn register_exception_callback<F>(&mut self, callback: F) -> Result<()>
    where
        F: Fn(ExceptionInfo<'_>) + Send + Sync + 'static,
    {
        let (callback, user) = into_user_data(callback);
        // SAFETY: trampoline 与 F 匹配；设备持有闭包到 `CloseDevice` 成功。
        unsafe {
            sdk_call!(MV3D_LP_RegisterExceptionCallBack(
                self.as_raw_handle(),
                Some(exception_trampoline::<F>),
                user
            ))
        }?;
        self.callbacks.push(callback);
        Ok(())
    }

    /// 关闭设备，返回 SDK 的结果。
    pub fn close(mut self) -> Result<()> {
        self.release()
    }

    pub(crate) const fn image_callback_registered(&self) -> bool {
        self.image_callback_registered
    }

    /// 记录已注册的 image callback；闭包保留到 Close。
    pub(crate) fn keep_image_callback(&mut self, callback: Arc<dyn Send + Sync>) {
        self.image_callback_registered = true;
        self.callbacks.push(callback);
    }

    /// 取走 handle 并 Close；`close` 之后的 `Drop` 因此不会重复释放。
    ///
    /// Close 失败时 SDK 可能仍持有闭包指针和会话资源，因此泄漏闭包与一份会话引用，
    /// Finalize 不再执行。
    fn release(&mut self) -> Result<()> {
        let Some(handle) = self.handle.take() else {
            return Ok(());
        };
        let mut handle = handle.as_ptr();
        // SAFETY: handle 由设备独占；借用设备的采集守卫都已释放。
        let result = unsafe { sdk_call!(MV3D_LP_CloseDevice(&raw mut handle)) };
        if result.is_err() {
            mem::forget(mem::take(&mut self.callbacks));
            mem::forget(Arc::clone(&self.session));
        }
        result
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let _ = self.release();
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("handle", &self.as_raw_handle())
            .field("callbacks", &self.callbacks.len())
            .finish_non_exhaustive()
    }
}

/// 借用两个文件名构造描述符；SDK 只在阻塞的传输调用期间读取它们。
fn file_access(local_file: &CStr, device_file: &CStr) -> sys::MV3D_LP_FILE_ACCESS {
    sys::MV3D_LP_FILE_ACCESS {
        pUserFileName: local_file.as_ptr(),
        pDevFileName: device_file.as_ptr(),
        ..Default::default()
    }
}
