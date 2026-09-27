//! SDK callback 的 trampoline 与回调参数类型。
//!
//! 注册时把 `Box<F>` 的地址作为 `pUser` 交给 SDK，trampoline 按注册时的具体类型 `F` 还原闭包，
//! 不需要锁或全局表。LPSDK 不能注销 callback，闭包由设备保留到 Close。
//! Rust 1.81 起，panic 越过 `extern "C"` 函数会直接终止进程。

use std::ffi::CStr;
use std::os::raw::c_void;

use crate::{Image, fixed_cstr, sys};

/// 设备异常类型。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExceptionKind {
    /// 设备断开连接（`DevExceptionType_Disconnect`）。
    Disconnected,
    /// 其它类型，含 `DevExceptionType_Undefined`。
    Other(i32),
}

/// exception callback 收到的异常，只在本次回调期间有效。
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct DeviceException<'a> {
    /// 异常类型。
    pub kind: ExceptionKind,
    /// SDK 给出的描述。
    pub description: &'a CStr,
}

/// image callback 的 trampoline：在回调返回前把图像复制为 [`Image`]。
///
/// # Safety
///
/// `user` 必须是注册时传入的 `Box<F>` 地址且闭包仍然存活；`image` 只在本次调用期间有效。
pub(crate) unsafe extern "C" fn image_trampoline<F>(
    image: *mut sys::MV3D_LP_IMAGE_DATA,
    user: *mut c_void,
) where
    F: Fn(Image),
{
    // SAFETY: 见函数的 Safety 约定。
    let (callback, image) = unsafe { (&*user.cast::<F>(), image.as_ref()) };
    if let Some(image) = image {
        // SAFETY: SDK 保证描述符中的缓冲区在本次回调返回前有效。
        callback(unsafe { Image::from_raw(image) });
    }
}

/// exception callback 的 trampoline。
///
/// # Safety
///
/// `user` 必须是注册时传入的 `Box<F>` 地址且闭包仍然存活；`info` 只在本次调用期间有效。
pub(crate) unsafe extern "C" fn exception_trampoline<F>(
    info: *mut sys::MV3D_LP_EXCEPTION_INFO,
    user: *mut c_void,
) where
    F: Fn(&DeviceException<'_>),
{
    // SAFETY: 见函数的 Safety 约定。
    let (callback, info) = unsafe { (&*user.cast::<F>(), info.as_ref()) };
    if let Some(info) = info {
        let kind = match info.enExceptionType {
            sys::DevExceptionType_Disconnect => ExceptionKind::Disconnected,
            other => ExceptionKind::Other(other),
        };
        callback(&DeviceException {
            kind,
            description: fixed_cstr(&info.chExceptionDesc),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::os::raw::c_void;
    use std::ptr;
    use std::sync::Mutex;

    use super::{DeviceException, ExceptionKind, exception_trampoline, image_trampoline};
    use crate::{Image, sys};

    fn user<F>(callback: &F) -> *mut c_void {
        ptr::from_ref(callback).cast_mut().cast()
    }

    unsafe fn fire_image<F: Fn(Image)>(callback: &F, image: &mut sys::MV3D_LP_IMAGE_DATA) {
        // SAFETY: 调用方保证闭包与描述符在同步调用期间有效。
        unsafe { image_trampoline::<F>(image, user(callback)) };
    }

    unsafe fn fire_exception<F: Fn(&DeviceException<'_>)>(
        callback: &F,
        info: &mut sys::MV3D_LP_EXCEPTION_INFO,
    ) {
        // SAFETY: 调用方保证闭包与描述符在同步调用期间有效。
        unsafe { exception_trampoline::<F>(info, user(callback)) };
    }

    // trampoline 按注册类型还原闭包，图像在回调内已被复制，异常类型与描述被转换。
    #[test]
    fn trampolines_restore_the_closure_and_convert_arguments() {
        let seen = Mutex::new(Vec::new());
        let mut data = [1_u8, 2];
        let mut image = sys::MV3D_LP_IMAGE_DATA {
            enImageType: sys::ImageType_Mono8,
            nWidth: 2,
            nHeight: 1,
            pData: data.as_mut_ptr(),
            nDataLen: 2,
            ..Default::default()
        };
        let mut info = sys::MV3D_LP_EXCEPTION_INFO {
            enExceptionType: sys::DevExceptionType_Disconnect,
            ..Default::default()
        };
        info.chExceptionDesc[0] = b'x'.cast_signed();

        // SAFETY: 闭包与描述符都是本函数的局部变量。
        unsafe {
            fire_image(
                &|image: Image| seen.lock().unwrap().push(format!("{:?}", image.data)),
                &mut image,
            );
            fire_exception(
                &|exception: &DeviceException<'_>| {
                    assert_eq!(exception.kind, ExceptionKind::Disconnected);
                    seen.lock()
                        .unwrap()
                        .push(format!("{:?}", exception.description));
                },
                &mut info,
            );
        }

        assert_eq!(*seen.lock().unwrap(), ["[1, 2]", "\"x\""]);
    }
}
