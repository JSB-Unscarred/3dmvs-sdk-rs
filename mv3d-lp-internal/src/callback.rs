#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::collections::HashMap;
use std::ffi::c_void;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::bindings;
use crate::driver::{DriverError, DriverResult};
use crate::frame::FrameRecord;

/// Opaque callback identifier passed through the SDK without dereferencing native user data.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CallbackCookie(NonZeroUsize);

impl CallbackCookie {
    pub(crate) fn as_user_pointer(self) -> *mut c_void {
        ptr::without_provenance_mut(self.0.get())
    }

    fn from_user_pointer(pointer: *mut c_void) -> Option<Self> {
        NonZeroUsize::new(pointer.addr()).map(Self)
    }
}

/// Returns `false` after the receiver disconnects so the registry can stop delivery.
pub type FrameCallbackSink = Arc<dyn Fn(FrameRecord) -> bool + Send + Sync + 'static>;
pub type ExceptionCallbackSink = Arc<dyn Fn(ExceptionRecord) -> bool + Send + Sync + 'static>;

/// Owned exception payload copied before the native callback returns.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExceptionRecord {
    pub kind: i32,
    pub description: Vec<u8>,
}

#[derive(Clone)]
enum CallbackSink {
    Image(FrameCallbackSink),
    Exception(ExceptionCallbackSink),
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CallbackKind {
    Image,
    Exception,
}

impl CallbackSink {
    fn kind(&self) -> CallbackKind {
        match self {
            Self::Image(_) => CallbackKind::Image,
            Self::Exception(_) => CallbackKind::Exception,
        }
    }
}

struct RegistryState {
    next_cookie: usize,
    entries: HashMap<CallbackCookie, CallbackSink>,
}

struct CallbackRegistry {
    state: Mutex<RegistryState>,
}

impl CallbackRegistry {
    fn new() -> Self {
        Self {
            state: Mutex::new(RegistryState {
                next_cookie: 1,
                entries: HashMap::new(),
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, RegistryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Inserts a sink under a never-reused cookie so a late callback cannot hit a newer sink.
    fn insert(&self, sink: CallbackSink) -> CallbackCookie {
        let mut state = self.lock();
        let cookie = CallbackCookie(
            NonZeroUsize::new(state.next_cookie).expect("callback cookie space exhausted"),
        );
        state.next_cookie = state
            .next_cookie
            .checked_add(1)
            .expect("callback cookie space exhausted");
        let previous = state.entries.insert(cookie, sink);
        debug_assert!(previous.is_none(), "callback cookies are never reused");
        cookie
    }

    /// Clones the sink so an admitted callback can finish if registration is removed concurrently.
    fn lookup(&self, cookie: CallbackCookie, expected: CallbackKind) -> Option<CallbackSink> {
        let sink = self.lock().entries.get(&cookie).cloned()?;
        (sink.kind() == expected).then_some(sink)
    }

    fn remove(&self, cookie: CallbackCookie) {
        self.lock().entries.remove(&cookie);
    }
}

fn registry() -> &'static CallbackRegistry {
    static REGISTRY: OnceLock<CallbackRegistry> = OnceLock::new();
    REGISTRY.get_or_init(CallbackRegistry::new)
}

/// Owns one registry entry while the corresponding native registration is usable.
pub(crate) struct CallbackRegistration {
    cookie: CallbackCookie,
}

impl CallbackRegistration {
    pub(crate) fn image(sink: FrameCallbackSink) -> Self {
        Self::new(CallbackSink::Image(sink))
    }

    pub(crate) fn exception(sink: ExceptionCallbackSink) -> Self {
        Self::new(CallbackSink::Exception(sink))
    }

    fn new(sink: CallbackSink) -> Self {
        Self {
            cookie: registry().insert(sink),
        }
    }

    pub(crate) fn cookie(&self) -> CallbackCookie {
        self.cookie
    }
}

impl Drop for CallbackRegistration {
    fn drop(&mut self) {
        registry().remove(self.cookie);
    }
}

/// Copies one image callback into an owned Rust value without unwinding through the C ABI.
pub(crate) unsafe extern "system" fn image_trampoline(
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    user: *mut c_void,
) {
    let cookie = CallbackCookie::from_user_pointer(user);
    if catch_unwind(AssertUnwindSafe(|| {
        if let Some(cookie) = cookie {
            dispatch_image(cookie, image);
        }
    }))
    .is_err()
    {
        if let Some(cookie) = cookie {
            registry().remove(cookie);
        }
    }
}

/// Copies one exception callback into an owned Rust value without unwinding through the C ABI.
pub(crate) unsafe extern "system" fn exception_trampoline(
    exception: *mut bindings::MV3D_LP_EXCEPTION_INFO,
    user: *mut c_void,
) {
    let cookie = CallbackCookie::from_user_pointer(user);
    if catch_unwind(AssertUnwindSafe(|| {
        if let Some(cookie) = cookie {
            dispatch_exception(cookie, exception);
        }
    }))
    .is_err()
    {
        if let Some(cookie) = cookie {
            registry().remove(cookie);
        }
    }
}

fn dispatch_image(cookie: CallbackCookie, image: *mut bindings::MV3D_LP_IMAGE_DATA) {
    let Some(CallbackSink::Image(sink)) = registry().lookup(cookie, CallbackKind::Image) else {
        return;
    };
    // SAFETY: the SDK owns the descriptor for the duration of this callback; null is ignored.
    let Some(image) = (unsafe { image.as_ref() }) else {
        return;
    };
    // SAFETY: the callback converter validates pointer/length pairs before copying every payload.
    if let Ok(frame) = unsafe { crate::ffi::callback_image_from_native(image) } {
        if !sink(frame) {
            registry().remove(cookie);
        }
    }
}

fn dispatch_exception(cookie: CallbackCookie, exception: *mut bindings::MV3D_LP_EXCEPTION_INFO) {
    let Some(CallbackSink::Exception(sink)) = registry().lookup(cookie, CallbackKind::Exception)
    else {
        return;
    };
    // SAFETY: the SDK owns the descriptor for the duration of this callback; null is ignored.
    let Some(exception) = (unsafe { exception.as_ref() }) else {
        return;
    };
    if let Ok(exception) = exception_from_native(exception) {
        if !sink(exception) {
            registry().remove(cookie);
        }
    }
}

fn exception_from_native(
    exception: &bindings::MV3D_LP_EXCEPTION_INFO,
) -> DriverResult<ExceptionRecord> {
    let length = exception
        .chExceptionDesc
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(exception.chExceptionDesc.len());
    let mut description = Vec::new();
    description
        .try_reserve_exact(length)
        .map_err(|_| DriverError::Allocation { requested: length })?;
    description.extend(
        exception.chExceptionDesc[..length]
            .iter()
            .map(|byte| *byte as u8),
    );
    Ok(ExceptionRecord {
        kind: exception.enExceptionType,
        description,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{CallbackRegistration, FrameCallbackSink, image_trampoline};
    use crate::{bindings, ffi};

    // 验证 callback 立即复制 payload；registration 销毁后迟到 callback 不再投递。
    #[test]
    fn image_callback_owns_payload_and_retires_registration() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::clone(&received);
        let sink: FrameCallbackSink = Arc::new(move |frame| {
            output.lock().unwrap().push(frame);
            true
        });
        let registration = CallbackRegistration::image(sink);
        let cookie = registration.cookie();

        let mut data = [1_u8, 2];
        let mut image = ffi::zeroed_image();
        image.enImageType = bindings::ImageType_Mono8;
        image.nWidth = 2;
        image.nHeight = 1;
        image.pData = data.as_mut_ptr();
        image.nDataLen = data.len() as u32;

        // SAFETY: the descriptor and its payload stay alive for this synchronous callback.
        unsafe { image_trampoline(&mut image, cookie.as_user_pointer()) };
        data.fill(9);
        assert_eq!(received.lock().unwrap()[0].data, [1, 2]);

        drop(registration);
        // SAFETY: the descriptor remains valid; the retired cookie is ignored before conversion.
        unsafe { image_trampoline(&mut image, cookie.as_user_pointer()) };
        assert_eq!(received.lock().unwrap().len(), 1);
    }
}
