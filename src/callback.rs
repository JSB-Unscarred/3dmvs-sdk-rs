//! SDK callback 的 trampoline 与回调参数类型。
//!
//! 闭包放在 `Arc` 中，`pUser` 是 `Arc::into_raw` 的地址，trampoline 按注册时的类型 `F` 还原闭包。
//! LPSDK 不能注销 callback，设备持有一份强引用到 `CloseDevice`；trampoline 每次调用期间再持有一份，
//! 所以在 callback 里释放设备也不会释放正在执行的闭包。前提是 `CloseDevice` 成功后 SDK 不再回调。
//! panic 越过 `extern "C"` 函数时进程终止（Rust 1.81 起）。

use std::ffi::CStr;
use std::ffi::c_void;
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

/// exception callback 收到的异常，只在本次回调中有效。
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
        // SAFETY: SDK 保证 buffer 在本次回调返回前有效。
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
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{ExceptionInfo, exception_trampoline, into_user_data};
    use crate::sys;

    static OWNER: Mutex<Option<Arc<dyn Send + Sync>>> = Mutex::new(None);

    // callback 在执行中释放 owner（相当于在回调里 drop 设备）时，闭包存活到本次调用返回，之后才释放。
    #[test]
    fn trampoline_keeps_the_closure_alive_for_the_call() {
        static ALIVE_AFTER_RELEASE: AtomicBool = AtomicBool::new(false);
        let captured = Arc::new(());
        let witness = Arc::clone(&captured);
        fire(move |_| {
            drop(OWNER.lock().unwrap().take());
            ALIVE_AFTER_RELEASE.store(Arc::strong_count(&captured) == 2, Ordering::SeqCst);
        });
        assert!(ALIVE_AFTER_RELEASE.load(Ordering::SeqCst));
        assert_eq!(Arc::strong_count(&witness), 1);
    }

    /// 与注册时相同地交出闭包并由 OWNER 持有，再像 SDK 一样调用一次 trampoline。
    fn fire<F: Fn(ExceptionInfo<'_>) + Send + Sync + 'static>(callback: F) {
        let (owner, user) = into_user_data(callback);
        *OWNER.lock().unwrap() = Some(owner);
        let mut info = sys::MV3D_LP_EXCEPTION_INFO::default();
        // SAFETY: user 来自 into_user_data::<F>，调用开始时 owner 仍在 OWNER 中；info 是局部变量。
        unsafe { exception_trampoline::<F>(&raw mut info, user) };
    }
}
