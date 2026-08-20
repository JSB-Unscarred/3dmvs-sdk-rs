#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::collections::HashMap;
use std::ffi::c_void;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use crate::bindings;
use crate::bits::bit_newtype;
use crate::frame::Image;
use crate::text::SdkText;

bit_newtype! {
    /// A device exception type reported by the SDK, preserving unknown values.
    pub struct DeviceExceptionType;
    UNDEFINED = 0xFFFF_FFFF => "undefined",
    DISCONNECTED = 0x0000_0001 => "disconnected",
}

/// An owned device exception delivered by the safe callback facade.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub struct DeviceException {
    pub kind: DeviceExceptionType,
    pub description: SdkText,
}

impl DeviceException {
    #[must_use]
    pub const fn new(kind: DeviceExceptionType, description: SdkText) -> Self {
        Self { kind, description }
    }
}

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

pub type ImageCallback = Arc<dyn Fn(Image) + Send + Sync + 'static>;
pub type ExceptionCallback = Arc<dyn Fn(DeviceException) + Send + Sync + 'static>;

#[derive(Clone)]
enum CallbackSink {
    Image(ImageCallback),
    Exception(ExceptionCallback),
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
    pub(crate) fn image(sink: ImageCallback) -> Self {
        Self::new(CallbackSink::Image(sink))
    }

    pub(crate) fn exception(sink: ExceptionCallback) -> Self {
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

pub(crate) unsafe extern "system" fn image_trampoline(
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    user: *mut c_void,
) {
    let Some(cookie) = CallbackCookie::from_user_pointer(user) else {
        return;
    };
    dispatch_image(cookie, image);
}

pub(crate) unsafe extern "system" fn exception_trampoline(
    exception: *mut bindings::MV3D_LP_EXCEPTION_INFO,
    user: *mut c_void,
) {
    let Some(cookie) = CallbackCookie::from_user_pointer(user) else {
        return;
    };
    dispatch_exception(cookie, exception);
}

fn dispatch_image(cookie: CallbackCookie, image: *mut bindings::MV3D_LP_IMAGE_DATA) {
    let Some(CallbackSink::Image(sink)) = registry().lookup(cookie, CallbackKind::Image) else {
        return;
    };
    // SAFETY: the SDK owns the descriptor for the duration of this callback.
    let Some(image) = (unsafe { image.as_ref() }) else {
        return;
    };
    // SAFETY: conversion validates pointer/length pairs before copying. Invalid layouts are skipped.
    let Ok(frame) = (unsafe { crate::ffi::callback_image_from_native(image) }) else {
        return;
    };
    if catch_unwind(AssertUnwindSafe(|| sink(frame))).is_err() {
        registry().remove(cookie);
    }
}

fn dispatch_exception(cookie: CallbackCookie, exception: *mut bindings::MV3D_LP_EXCEPTION_INFO) {
    let Some(CallbackSink::Exception(sink)) = registry().lookup(cookie, CallbackKind::Exception)
    else {
        return;
    };
    // SAFETY: the SDK owns the descriptor for the duration of this callback.
    let Some(exception) = (unsafe { exception.as_ref() }) else {
        return;
    };
    let event = exception_from_native(exception);
    if catch_unwind(AssertUnwindSafe(|| sink(event))).is_err() {
        registry().remove(cookie);
    }
}

fn exception_from_native(exception: &bindings::MV3D_LP_EXCEPTION_INFO) -> DeviceException {
    let length = exception
        .chExceptionDesc
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(exception.chExceptionDesc.len());
    let description = exception.chExceptionDesc[..length]
        .iter()
        .map(|byte| *byte as u8)
        .collect();
    DeviceException::new(
        DeviceExceptionType::from_raw(exception.enExceptionType),
        SdkText::from_sdk_bytes(description),
    )
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{CallbackRegistration, ImageCallback, image_trampoline};
    use crate::{bindings, ffi};

    // 验证 callback 立即复制 payload；registration 销毁后迟到 callback 不再投递。
    #[test]
    fn image_callback_owns_payload_and_retires_registration() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let output = Arc::clone(&received);
        let sink: ImageCallback = Arc::new(move |frame| {
            output.lock().unwrap().push(frame);
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
