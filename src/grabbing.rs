//! 采集状态：pull 与 callback 两种模式各对应一个借用设备的守卫。
//!
//! 守卫可变借用 [`Device`]，因此采集期间无法再次开始采集、注册 callback 或关闭设备；
//! 参数读写经 `Deref` 仍可使用。守卫释放时停止采集，需要观察错误时调用 `stop`。

use std::mem;
use std::ops::Deref;
use std::time::Duration;

use crate::callback::{image_trampoline, into_user_data};
use crate::error::sdk_call;
use crate::{Device, Error, Image, Result, sys};

impl Device {
    /// 以 pull 模式开始采集，之后用 [`Grabbing::get_image`] 取图。
    ///
    /// 注册过 image callback 的设备在 Close 前返回 [`Error::ImageCallbackRegistered`]。
    pub fn start_grabbing(&mut self) -> Result<Grabbing<'_>> {
        if self.image_callback_registered() {
            return Err(Error::ImageCallbackRegistered);
        }
        // SAFETY: 可变借用保证当前没有其它采集守卫。
        unsafe { sdk_call!(MV3D_LP_StartMeasure(self.as_raw_handle())) }?;
        Ok(Grabbing { device: self })
    }

    /// 注册 image callback 并开始采集。
    ///
    /// SDK 在内部线程调用 `callback`，图像在回调返回前已复制为 [`Image`]。LPSDK 不能注销
    /// callback，闭包保留到 Close，此后该设备不能再用 pull 取图。callback 内的 panic 会在
    /// FFI 边界终止进程。
    pub fn start_grabbing_with<F>(&mut self, callback: F) -> Result<CallbackGrabbing<'_>>
    where
        F: Fn(Image) + Send + Sync + 'static,
    {
        let handle = self.as_raw_handle();
        let (callback, user) = into_user_data(callback);
        // SAFETY: trampoline 与 F 匹配；设备持有闭包到 `CloseDevice` 成功。
        unsafe {
            sdk_call!(MV3D_LP_RegisterImageDataCallBack(
                handle,
                Some(image_trampoline::<F>),
                user
            ))
        }?;
        self.keep_image_callback(callback);
        // SAFETY: 可变借用保证当前没有其它采集守卫。
        unsafe { sdk_call!(MV3D_LP_StartMeasure(handle)) }?;
        Ok(CallbackGrabbing { device: self })
    }
}

/// pull 模式的采集守卫。
#[derive(Debug)]
#[must_use = "grabbing stops when the guard is dropped"]
pub struct Grabbing<'a> {
    device: &'a mut Device,
}

impl Grabbing<'_> {
    /// 等待一帧并复制为 [`Image`]；`None` 表示无限等待。
    pub fn get_image(&self, timeout: Option<Duration>) -> Result<Image> {
        let mut raw = sys::MV3D_LP_IMAGE_DATA::default();
        // SAFETY: raw 是清零的可写输出。
        unsafe {
            sdk_call!(MV3D_LP_GetImage(
                self.device.as_raw_handle(),
                &raw mut raw,
                timeout_ms(timeout)
            ))
        }?;
        // SAFETY: 输出缓冲区在下一次 GetImage 前有效；守卫不是 Sync，复制期间不会有其它取图调用。
        Ok(unsafe { Image::from_raw(&raw) })
    }

    /// 停止采集并返回 SDK 的结果。
    pub fn stop(self) -> Result<()> {
        let result = stop_measure(self.device);
        mem::forget(self);
        result
    }
}

impl Deref for Grabbing<'_> {
    type Target = Device;

    fn deref(&self) -> &Device {
        self.device
    }
}

impl Drop for Grabbing<'_> {
    fn drop(&mut self) {
        let _ = stop_measure(self.device);
    }
}

/// callback 模式的采集守卫。
#[derive(Debug)]
#[must_use = "grabbing stops when the guard is dropped"]
pub struct CallbackGrabbing<'a> {
    device: &'a mut Device,
}

impl CallbackGrabbing<'_> {
    /// 停止采集并返回 SDK 的结果。
    pub fn stop(self) -> Result<()> {
        let result = stop_measure(self.device);
        mem::forget(self);
        result
    }
}

impl Deref for CallbackGrabbing<'_> {
    type Target = Device;

    fn deref(&self) -> &Device {
        self.device
    }
}

impl Drop for CallbackGrabbing<'_> {
    fn drop(&mut self) {
        let _ = stop_measure(self.device);
    }
}

fn stop_measure(device: &Device) -> Result<()> {
    // SAFETY: 调用方是该设备唯一的采集守卫。
    unsafe { sdk_call!(MV3D_LP_StopMeasure(device.as_raw_handle())) }
}

/// 转换为 SDK 的毫秒等待时间：`u32::MAX` 是无限等待，有限等待最多取 `u32::MAX - 1`。
fn timeout_ms(timeout: Option<Duration>) -> u32 {
    timeout.map_or(u32::MAX, |timeout| {
        u32::try_from(timeout.as_millis()).map_or(u32::MAX - 1, |ms| ms.min(u32::MAX - 1))
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::timeout_ms;

    // 有限等待不会退化成 SDK 的无限等待哨兵。
    #[test]
    fn finite_timeouts_never_become_infinite() {
        assert_eq!(timeout_ms(None), u32::MAX);
        assert_eq!(timeout_ms(Some(Duration::ZERO)), 0);
        assert_eq!(timeout_ms(Some(Duration::MAX)), u32::MAX - 1);
    }
}
