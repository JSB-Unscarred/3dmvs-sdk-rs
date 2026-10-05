//! 取流状态：pull 与 callback 两种模式各对应一个守卫。
//!
//! 守卫对设备的持有方式泛型（[`HoldsDevice`]）：`&mut Device` 借用设备，适合在一个作用域内取流；
//! `Device` 按值持有，守卫可以存进结构体，`stop` 后交还设备。两种方式下取流期间都无法再次开始取流、
//! 注册 callback 或关闭设备；参数读写经 `Deref` 仍可使用。守卫释放时停止取流，需要观察错误时调用 `stop`。

use std::borrow::BorrowMut;
use std::mem::ManuallyDrop;
use std::ops::Deref;
use std::ptr;
use std::time::Duration;

use crate::callback::{image_trampoline, into_user_data};
use crate::error::sdk_call;
use crate::{Device, Error, Image, Result, sys};

/// 取流守卫持有设备的方式：`&mut Device` 借用，`Device` 按值持有。
///
/// sealed trait：守卫依赖每次借出的都是开始取流的同一台设备，任意 `BorrowMut` 实现无法保证这一点。
pub trait HoldsDevice: sealed::Sealed + BorrowMut<Device> {}

impl HoldsDevice for Device {}
impl HoldsDevice for &mut Device {}

mod sealed {
    use crate::Device;

    pub trait Sealed {}

    impl Sealed for Device {}
    impl Sealed for &mut Device {}
}

impl Device {
    /// 借用设备开始主动取图，等同于 [`Grabbing::start`]；按值持有时直接调用后者。
    pub fn start_grabbing(&mut self) -> Result<Grabbing<&mut Self>> {
        Grabbing::start(self).map_err(|(_, error)| error)
    }

    /// 借用设备注册 image callback 并开始取流，等同于 [`CallbackGrabbing::start`]；按值持有时直接调用后者。
    pub fn start_grabbing_with<F>(&mut self, callback: F) -> Result<CallbackGrabbing<&mut Self>>
    where
        F: Fn(Image) + Send + Sync + 'static,
    {
        CallbackGrabbing::start(self, callback).map_err(|(_, error)| error)
    }
}

/// 主动取图的取流守卫，释放时停止取流。
#[derive(Debug)]
#[must_use = "grabbing stops when the guard is dropped"]
pub struct Grabbing<D: HoldsDevice> {
    device: D,
}

impl<D: HoldsDevice> Grabbing<D> {
    /// 开始主动取图，之后用 [`Grabbing::get_image`] 取图；失败时交还设备。
    ///
    /// 注册过 image callback 的设备在关闭前返回 [`Error::ImageCallbackRegistered`]。
    pub fn start(device: D) -> std::result::Result<Self, (D, Error)> {
        if device.borrow().image_callback_registered() {
            return Err((device, Error::ImageCallbackRegistered));
        }
        // SAFETY: 守卫独占设备，当前没有其它取流守卫。
        match unsafe { sdk_call!(MV3D_LP_StartMeasure(device.borrow().as_raw_handle())) } {
            Ok(()) => Ok(Self { device }),
            Err(error) => Err((device, error)),
        }
    }

    /// 等待一帧并复制为 [`Image`]，`None` 表示无限等待；超时返回 [`ErrorCode::NoData`](crate::ErrorCode::NoData)。
    pub fn get_image(&self, timeout: Option<Duration>) -> Result<Image> {
        let mut raw = sys::MV3D_LP_IMAGE_DATA::default();
        // SAFETY: raw 是可写输出。
        unsafe {
            sdk_call!(MV3D_LP_GetImage(
                self.device.borrow().as_raw_handle(),
                &raw mut raw,
                timeout_ms(timeout)
            ))
        }?;
        // SAFETY: 输出 buffer 在下一次 GetImage 前有效；守卫不是 Sync，复制期间不会有其它取图调用。
        Ok(unsafe { Image::from_raw(&raw) })
    }

    /// 停止取流，交还设备与 SDK 的结果。
    pub fn stop(self) -> (D, Result<()>) {
        let this = ManuallyDrop::new(self);
        let result = stop_grabbing(this.device.borrow());
        // SAFETY: this 不再使用也不会 drop，device 只读出这一次。
        (unsafe { ptr::read(&raw const this.device) }, result)
    }
}

impl<D: HoldsDevice> Deref for Grabbing<D> {
    type Target = Device;

    fn deref(&self) -> &Device {
        self.device.borrow()
    }
}

impl<D: HoldsDevice> Drop for Grabbing<D> {
    fn drop(&mut self) {
        let _ = stop_grabbing(self.device.borrow());
    }
}

/// callback 取图的取流守卫，释放时停止取流。
#[derive(Debug)]
#[must_use = "grabbing stops when the guard is dropped"]
pub struct CallbackGrabbing<D: HoldsDevice> {
    device: D,
}

impl<D: HoldsDevice> CallbackGrabbing<D> {
    /// 注册 image callback 并开始取流；失败时交还设备。
    ///
    /// `callback` 在 SDK 的线程中运行，其中的 panic 会终止进程；收到的 [`Image`] 已经复制出来。
    ///
    /// LPSDK 不能注销 image callback：闭包保留到设备关闭，这台设备此后不能再主动取图。注册成功而
    /// `MV3D_LP_StartMeasure` 失败时也是如此，此时交还设备，可以再次调用本方法重试。
    pub fn start<F>(mut device: D, callback: F) -> std::result::Result<Self, (D, Error)>
    where
        F: Fn(Image) + Send + Sync + 'static,
    {
        let handle = device.borrow().as_raw_handle();
        let (callback, user) = into_user_data(callback);
        // SAFETY: trampoline 与 F 匹配；设备持有闭包到 `CloseDevice` 成功。
        let registered = unsafe {
            sdk_call!(MV3D_LP_RegisterImageDataCallBack(
                handle,
                Some(image_trampoline::<F>),
                user
            ))
        };
        if let Err(error) = registered {
            return Err((device, error));
        }
        device.borrow_mut().keep_image_callback(callback);
        // SAFETY: 守卫独占设备，当前没有其它取流守卫。
        match unsafe { sdk_call!(MV3D_LP_StartMeasure(handle)) } {
            Ok(()) => Ok(Self { device }),
            Err(error) => Err((device, error)),
        }
    }

    /// 停止取流，交还设备与 SDK 的结果。
    pub fn stop(self) -> (D, Result<()>) {
        let this = ManuallyDrop::new(self);
        let result = stop_grabbing(this.device.borrow());
        // SAFETY: this 不再使用也不会 drop，device 只读出这一次。
        (unsafe { ptr::read(&raw const this.device) }, result)
    }
}

impl<D: HoldsDevice> Deref for CallbackGrabbing<D> {
    type Target = Device;

    fn deref(&self) -> &Device {
        self.device.borrow()
    }
}

impl<D: HoldsDevice> Drop for CallbackGrabbing<D> {
    fn drop(&mut self) {
        let _ = stop_grabbing(self.device.borrow());
    }
}

/// 停止取流；调用方是该设备唯一的取流守卫。
fn stop_grabbing(device: &Device) -> Result<()> {
    // SAFETY: 调用方是该设备唯一的取流守卫。
    unsafe { sdk_call!(MV3D_LP_StopMeasure(device.as_raw_handle())) }
}

/// 转换为 SDK 的毫秒等待时间：`u32::MAX` 是无限等待，有限等待最多取 `u32::MAX - 1`；
/// 不足 1 毫秒的部分向上取整，与 std 在 Windows 上的超时换算一致，避免短等待退化为不等待。
fn timeout_ms(timeout: Option<Duration>) -> u32 {
    timeout.map_or(u32::MAX, |timeout| {
        u32::try_from(timeout.as_nanos().div_ceil(1_000_000))
            .map_or(u32::MAX - 1, |ms| ms.min(u32::MAX - 1))
    })
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::timeout_ms;

    // 有限等待不会退化成 SDK 的无限等待哨兵，亚毫秒等待向上取整。
    #[test]
    fn finite_timeouts_never_become_infinite() {
        assert_eq!(timeout_ms(None), u32::MAX);
        assert_eq!(timeout_ms(Some(Duration::ZERO)), 0);
        assert_eq!(timeout_ms(Some(Duration::from_micros(1))), 1);
        assert_eq!(timeout_ms(Some(Duration::MAX)), u32::MAX - 1);
    }
}
