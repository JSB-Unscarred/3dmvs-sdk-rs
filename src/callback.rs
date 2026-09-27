//! SDK callback 的 trampoline 与回调参数类型。
//!
//! 注册时把闭包放入 `Arc`，以 `Arc::into_raw` 的地址作为 `pUser` 交给 SDK，trampoline 按注册时的
//! 具体类型 `F` 还原闭包，不需要锁或全局表。LPSDK 不能注销 callback，设备持有一份强引用到
//! `CloseDevice`；trampoline 在每次调用期间再持有一份，callback 中释放设备时，正在执行的闭包因此
//! 存活到调用返回。Rust 1.81 起，panic 越过 `extern "C"` 函数会直接终止进程。

use std::ffi::CStr;
use std::os::raw::c_void;
use std::sync::Arc;

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

impl ExceptionKind {
    /// 由 SDK 原始值构造，未定义的值保存在 [`ExceptionKind::Other`]。
    const fn from_raw(raw: i32) -> Self {
        match raw {
            sys::DevExceptionType_Disconnect => Self::Disconnected,
            other => Self::Other(other),
        }
    }
}

/// exception callback 收到的异常，只在本次回调期间有效。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ExceptionInfo<'a> {
    /// 异常类型。
    pub kind: ExceptionKind,
    /// SDK 给出的描述。
    pub description: &'a CStr,
}

/// 把闭包放入 `Arc`，返回 owner 持有的强引用与交给 SDK 的 `pUser`。
///
/// owner 只在 SDK 不再以该 `pUser` 回调后释放这份引用，清理失败时泄漏它。
pub(crate) fn into_user_data<F>(callback: F) -> (Arc<dyn Send + Sync>, *mut c_void)
where
    F: Send + Sync + 'static,
{
    let user = Arc::into_raw(Arc::new(callback));
    // SAFETY: user 刚由 into_raw 返回，取回的强引用交给 owner，SDK 只拿到地址。
    (unsafe { Arc::from_raw(user) }, user.cast_mut().cast())
}

/// 取回闭包，并在本次回调期间增持一份强引用：闭包在执行中释放 owner（例如 drop 设备）时，
/// 仍存活到调用返回。
///
/// # Safety
///
/// `user` 来自 [`into_user_data::<F>`]，且调用时 owner 的强引用尚未释放。
unsafe fn from_user_data<F>(user: *mut c_void) -> Arc<F> {
    let user = user.cast_const().cast::<F>();
    // SAFETY: 见函数的 Safety 约定。
    unsafe {
        Arc::increment_strong_count(user);
        Arc::from_raw(user)
    }
}

/// image callback 的 trampoline：在回调返回前把图像复制为 [`Image`]。
///
/// # Safety
///
/// `user` 来自 [`into_user_data::<F>`]，SDK 在 `CloseDevice` 成功后不再以它回调；
/// `image` 只在本次调用期间有效。
pub(crate) unsafe extern "C" fn image_trampoline<F>(
    image: *mut sys::MV3D_LP_IMAGE_DATA,
    user: *mut c_void,
) where
    F: Fn(Image),
{
    // SAFETY: 见函数的 Safety 约定。
    let (callback, image) = unsafe { (from_user_data::<F>(user), image.as_ref()) };
    if let Some(image) = image {
        // SAFETY: SDK 保证描述符中的缓冲区在本次回调返回前有效。
        callback(unsafe { Image::from_raw(image) });
    }
}

/// exception callback 的 trampoline。
///
/// # Safety
///
/// 同 [`image_trampoline`]；`info` 只在本次调用期间有效。
pub(crate) unsafe extern "C" fn exception_trampoline<F>(
    info: *mut sys::MV3D_LP_EXCEPTION_INFO,
    user: *mut c_void,
) where
    F: Fn(ExceptionInfo<'_>),
{
    // SAFETY: 见函数的 Safety 约定。
    let (callback, info) = unsafe { (from_user_data::<F>(user), info.as_ref()) };
    if let Some(info) = info {
        callback(ExceptionInfo {
            kind: ExceptionKind::from_raw(info.enExceptionType),
            description: fixed_cstr(&info.chExceptionDesc),
        });
    }
}

#[cfg(test)]
mod tests {
    use std::os::raw::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{ExceptionInfo, exception_trampoline, image_trampoline, into_user_data};
    use crate::{Image, sys};

    // 与注册时相同：返回 owner 的强引用、交给 SDK 的 callback 与 pUser。
    fn register_image<F>(
        callback: F,
    ) -> (
        Arc<dyn Send + Sync>,
        sys::MV3D_LP_ImageDataCallBack,
        *mut c_void,
    )
    where
        F: Fn(Image) + Send + Sync + 'static,
    {
        let (owner, user) = into_user_data(callback);
        (owner, Some(image_trampoline::<F>), user)
    }

    fn register_exception<F>(
        callback: F,
    ) -> (
        Arc<dyn Send + Sync>,
        sys::MV3D_LP_ExceptionCallBack,
        *mut c_void,
    )
    where
        F: Fn(ExceptionInfo<'_>) + Send + Sync + 'static,
    {
        let (owner, user) = into_user_data(callback);
        (owner, Some(exception_trampoline::<F>), user)
    }

    // trampoline 按注册类型还原闭包，图像在回调内已被复制，异常类型与描述被转换。
    #[test]
    fn trampolines_restore_the_closure_and_convert_arguments() {
        static SEEN: Mutex<Vec<String>> = Mutex::new(Vec::new());
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
        let (_image_owner, on_image, image_user) =
            register_image(|image| SEEN.lock().unwrap().push(format!("{:?}", image.data)));
        let (_exception_owner, on_exception, exception_user) = register_exception(|exception| {
            let seen = format!("{:?} {:?}", exception.kind, exception.description);
            SEEN.lock().unwrap().push(seen);
        });

        // SAFETY: owner 在同步调用期间存活，描述符是本函数的局部变量。
        unsafe {
            on_image.unwrap()(&raw mut image, image_user);
            on_exception.unwrap()(&raw mut info, exception_user);
        }

        assert_eq!(*SEEN.lock().unwrap(), ["[1, 2]", "Disconnected \"x\""]);
    }

    // callback 在执行中释放 owner（相当于在回调里 drop 设备）时，闭包存活到本次调用返回。
    #[test]
    fn trampoline_keeps_the_closure_alive_for_the_call() {
        static OWNER: Mutex<Option<Arc<dyn Send + Sync>>> = Mutex::new(None);
        static DROPPED: AtomicBool = AtomicBool::new(false);
        static ALIVE_AFTER_RELEASE: AtomicBool = AtomicBool::new(false);

        // 闭包捕获的探针，释放时置位 DROPPED。
        struct Probe(&'static AtomicBool);
        impl Probe {
            fn alive(&self) -> bool {
                !self.0.load(Ordering::SeqCst)
            }
        }
        impl Drop for Probe {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let probe = Probe(&DROPPED);
        let mut info = sys::MV3D_LP_EXCEPTION_INFO::default();
        let (owner, on_exception, user) = register_exception(move |_| {
            drop(OWNER.lock().unwrap().take());
            ALIVE_AFTER_RELEASE.store(probe.alive(), Ordering::SeqCst);
        });
        *OWNER.lock().unwrap() = Some(owner);
        // SAFETY: 调用开始时 owner 仍在 OWNER 中，info 是本函数的局部变量。
        unsafe { on_exception.unwrap()(&raw mut info, user) };

        assert!(ALIVE_AFTER_RELEASE.load(Ordering::SeqCst));
        assert!(DROPPED.load(Ordering::SeqCst));
    }
}
