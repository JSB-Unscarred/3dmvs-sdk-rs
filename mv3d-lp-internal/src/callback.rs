#![cfg_attr(not(feature = "native"), allow(dead_code))]

use std::collections::HashMap;
use std::ffi::c_void;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};

use crate::bindings;
use crate::driver::{DriverError, DriverResult};
use crate::error::{ContractViolation, Error, Operation};
use crate::frame::FrameRecord;

const REGISTER_IMAGE_OPERATION: Operation = Operation::RegisterImageDataCallback;
const REGISTER_EXCEPTION_OPERATION: Operation = Operation::RegisterExceptionCallback;
const MAX_COOKIE: usize = isize::MAX as usize;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct CallbackCookie(NonZeroUsize);

impl CallbackCookie {
    pub(crate) fn as_user_pointer(self) -> *mut c_void {
        ptr::without_provenance_mut(self.0.get())
    }

    fn from_user_pointer(pointer: *mut c_void) -> Option<Self> {
        NonZeroUsize::new(pointer.addr()).map(Self)
    }

    #[cfg(test)]
    pub(crate) const fn get(self) -> usize {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallbackDelivery {
    Delivered,
    Full,
    Disconnected,
}

pub type FrameCallbackSink = Arc<dyn Fn(FrameRecord) -> CallbackDelivery + Send + Sync + 'static>;
pub type ExceptionCallbackSink =
    Arc<dyn Fn(ExceptionRecord) -> CallbackDelivery + Send + Sync + 'static>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExceptionRecord {
    pub kind: i32,
    pub description: Vec<u8>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CallbackStatsRecord {
    pub delivered: u64,
    pub dropped_full: u64,
    pub invalid_payloads: u64,
    pub panics: u64,
    pub accepting: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CallbackKind {
    Image,
    Exception,
}

#[derive(Clone)]
enum CallbackSink {
    Image(FrameCallbackSink),
    Exception(ExceptionCallbackSink),
}

impl CallbackSink {
    fn kind(&self) -> CallbackKind {
        match self {
            Self::Image(_) => CallbackKind::Image,
            Self::Exception(_) => CallbackKind::Exception,
        }
    }
}

struct EntryState {
    in_flight: usize,
    // Presence is the callback's admission state as well as the owned root sink.
    sink: Option<CallbackSink>,
}

struct CallbackEntry {
    state: Mutex<EntryState>,
    drained: Condvar,
    delivered: AtomicU64,
    dropped_full: AtomicU64,
    invalid_payloads: AtomicU64,
    panics: AtomicU64,
}

impl CallbackEntry {
    fn new(sink: CallbackSink) -> Self {
        Self {
            state: Mutex::new(EntryState {
                in_flight: 0,
                sink: Some(sink),
            }),
            drained: Condvar::new(),
            delivered: AtomicU64::new(0),
            dropped_full: AtomicU64::new(0),
            invalid_payloads: AtomicU64::new(0),
            panics: AtomicU64::new(0),
        }
    }

    fn lock(&self) -> MutexGuard<'_, EntryState> {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn try_enter(self: &Arc<Self>, expected: CallbackKind) -> Option<InFlight> {
        let mut state = self.lock();
        if state.sink.as_ref().map(CallbackSink::kind) != Some(expected) {
            return None;
        }
        let Some(in_flight) = state.in_flight.checked_add(1) else {
            let retired_sink = state.sink.take();
            drop(state);
            increment_saturating(&self.panics);
            self.drop_sink_without_unwind(retired_sink);
            return None;
        };
        let sink = state.sink.as_ref()?.clone();
        state.in_flight = in_flight;
        drop(state);
        Some(InFlight {
            entry: Arc::clone(self),
            sink: Some(sink),
        })
    }

    fn begin_deactivate(&self) -> Option<CallbackSink> {
        let mut state = self.lock();
        state.sink.take()
    }

    fn wait_until_drained(&self) {
        let mut state = self.lock();
        while state.in_flight != 0 {
            state = self
                .drained
                .wait(state)
                .unwrap_or_else(|poisoned| poisoned.into_inner());
        }
    }

    fn fail_closed(&self, panic: bool) {
        if panic {
            increment_saturating(&self.panics);
        }
        let retired_sink = self.begin_deactivate();
        self.drop_sink_without_unwind(retired_sink);
    }

    fn drop_sink_without_unwind(&self, sink: Option<CallbackSink>) {
        let Some(sink) = sink else {
            return;
        };
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(sink))) {
            increment_saturating(&self.panics);
            // A custom panic payload may itself panic from Drop. Leaking only this exceptional
            // payload keeps user-controlled destruction from escaping callback cleanup.
            std::mem::forget(payload);
        }
    }

    fn record_invalid_payload(&self) {
        increment_saturating(&self.invalid_payloads);
    }

    fn record_delivery(&self, delivery: CallbackDelivery) {
        match delivery {
            CallbackDelivery::Delivered => increment_saturating(&self.delivered),
            CallbackDelivery::Full => increment_saturating(&self.dropped_full),
            CallbackDelivery::Disconnected => self.fail_closed(false),
        }
    }

    fn stats(&self) -> CallbackStatsRecord {
        let accepting = self.lock().sink.is_some();
        CallbackStatsRecord {
            delivered: self.delivered.load(Ordering::Relaxed),
            dropped_full: self.dropped_full.load(Ordering::Relaxed),
            invalid_payloads: self.invalid_payloads.load(Ordering::Relaxed),
            panics: self.panics.load(Ordering::Relaxed),
            accepting,
        }
    }
}

impl Drop for CallbackEntry {
    fn drop(&mut self) {
        // Registration normally removes the sink first. This final guard also covers early
        // construction failures and future paths that release the last entry directly.
        let sink = self
            .state
            .get_mut()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .sink
            .take();
        self.drop_sink_without_unwind(sink);
    }
}

struct InFlight {
    entry: Arc<CallbackEntry>,
    sink: Option<CallbackSink>,
}

impl Drop for InFlight {
    fn drop(&mut self) {
        // Release the callback clone before publishing that this invocation has drained. The
        // registration retains its own clone until wait_until_drained returns, so whichever clone
        // is last is always destroyed behind a panic boundary and never while holding state.
        let sink = self.sink.take();
        self.entry.drop_sink_without_unwind(sink);

        let mut state = self.entry.lock();
        if state.in_flight == 0 {
            let retired_sink = state.sink.take();
            drop(state);
            increment_saturating(&self.entry.panics);
            self.entry.drop_sink_without_unwind(retired_sink);
            return;
        }
        state.in_flight -= 1;
        if state.in_flight == 0 {
            self.entry.drained.notify_all();
        }
    }
}

struct RegistryState {
    next_cookie: usize,
    entries: HashMap<CallbackCookie, Arc<CallbackEntry>>,
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

    fn insert(&self, entry: Arc<CallbackEntry>) -> Option<CallbackCookie> {
        let mut state = self.lock();
        let cookie = NonZeroUsize::new(state.next_cookie).map(CallbackCookie)?;
        state.next_cookie = if state.next_cookie == MAX_COOKIE {
            0
        } else {
            state.next_cookie.checked_add(1)?
        };
        if state.entries.contains_key(&cookie) {
            return None;
        }
        state.entries.insert(cookie, entry);
        Some(cookie)
    }

    fn lookup(&self, cookie: CallbackCookie) -> Option<Arc<CallbackEntry>> {
        self.lock().entries.get(&cookie).cloned()
    }

    fn remove(&self, cookie: CallbackCookie, expected: &Arc<CallbackEntry>) {
        let mut state = self.lock();
        if state
            .entries
            .get(&cookie)
            .is_some_and(|entry| Arc::ptr_eq(entry, expected))
        {
            state.entries.remove(&cookie);
        }
    }
}

fn registry() -> &'static CallbackRegistry {
    static REGISTRY: OnceLock<CallbackRegistry> = OnceLock::new();
    REGISTRY.get_or_init(CallbackRegistry::new)
}

pub(crate) struct CallbackRegistration {
    cookie: CallbackCookie,
    entry: Arc<CallbackEntry>,
}

impl CallbackRegistration {
    pub(crate) fn image(sink: FrameCallbackSink) -> Result<Self, Error> {
        Self::new(REGISTER_IMAGE_OPERATION, CallbackSink::Image(sink))
    }

    pub(crate) fn exception(sink: ExceptionCallbackSink) -> Result<Self, Error> {
        Self::new(REGISTER_EXCEPTION_OPERATION, CallbackSink::Exception(sink))
    }

    fn new(operation: Operation, sink: CallbackSink) -> Result<Self, Error> {
        let entry = Arc::new(CallbackEntry::new(sink));
        let cookie = registry()
            .insert(Arc::clone(&entry))
            .ok_or(Error::ContractViolation {
                operation,
                kind: ContractViolation::CallbackCookieExhausted,
            })?;
        Ok(Self { cookie, entry })
    }

    pub(crate) fn cookie(&self) -> CallbackCookie {
        self.cookie
    }

    pub(crate) fn stats(&self) -> CallbackStatsRecord {
        self.entry.stats()
    }
}

impl Drop for CallbackRegistration {
    fn drop(&mut self) {
        // Revoke admission before hiding the cookie, then drain every callback that already
        // cloned the sink before destroying the registration's root clone.
        let retired_sink = self.entry.begin_deactivate();
        registry().remove(self.cookie, &self.entry);
        self.entry.wait_until_drained();
        self.entry.drop_sink_without_unwind(retired_sink);
    }
}

pub(crate) unsafe extern "system" fn image_trampoline(
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
    user: *mut c_void,
) {
    let cookie = CallbackCookie::from_user_pointer(user);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if let Some(cookie) = cookie {
            dispatch_image(cookie, image);
        }
    }));
    if let Err(payload) = outcome {
        if let Some(cookie) = cookie {
            fault_cookie_without_unwind(cookie);
        }
        // A custom panic payload may itself panic from Drop. Leaking only this exceptional
        // payload ensures even that destructor cannot unwind across the native ABI boundary.
        std::mem::forget(payload);
    }
}

pub(crate) unsafe extern "system" fn exception_trampoline(
    exception: *mut bindings::MV3D_LP_EXCEPTION_INFO,
    user: *mut c_void,
) {
    let cookie = CallbackCookie::from_user_pointer(user);
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        if let Some(cookie) = cookie {
            dispatch_exception(cookie, exception);
        }
    }));
    if let Err(payload) = outcome {
        if let Some(cookie) = cookie {
            fault_cookie_without_unwind(cookie);
        }
        // See image_trampoline: no panic-payload destructor may cross this ABI boundary.
        std::mem::forget(payload);
    }
}

fn dispatch_image(cookie: CallbackCookie, image: *mut bindings::MV3D_LP_IMAGE_DATA) {
    let Some(entry) = registry().lookup(cookie) else {
        return;
    };
    let Some(in_flight) = entry.try_enter(CallbackKind::Image) else {
        return;
    };
    let Some(CallbackSink::Image(sink)) = in_flight.sink.as_ref() else {
        return;
    };
    // SAFETY: after registry admission the audited SDK callback contract guarantees that the
    // descriptor pointer, when non-null, remains readable until this trampoline returns.
    let Some(image) = (unsafe { image.as_ref() }) else {
        entry.record_invalid_payload();
        return;
    };
    // SAFETY: admission proves this is an active SDK callback. The audited callback contract
    // requires the descriptor and every validated payload to remain readable until the native
    // trampoline returns. The conversion validates all extents before dereferencing payloads.
    match unsafe { crate::ffi::callback_image_from_native(image) } {
        Ok(frame) => entry.record_delivery(sink(frame)),
        Err(_) => entry.record_invalid_payload(),
    }
}

fn dispatch_exception(cookie: CallbackCookie, exception: *mut bindings::MV3D_LP_EXCEPTION_INFO) {
    let Some(entry) = registry().lookup(cookie) else {
        return;
    };
    let Some(in_flight) = entry.try_enter(CallbackKind::Exception) else {
        return;
    };
    let Some(CallbackSink::Exception(sink)) = in_flight.sink.as_ref() else {
        return;
    };
    // SAFETY: after registry admission the SDK owns a readable exception descriptor for the
    // duration of this callback. A null pointer remains a recoverable invalid event.
    let Some(exception) = (unsafe { exception.as_ref() }) else {
        entry.record_invalid_payload();
        return;
    };
    match exception_from_native(exception) {
        Ok(exception) => entry.record_delivery(sink(exception)),
        Err(_) => entry.record_invalid_payload(),
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

fn fault_cookie(cookie: CallbackCookie) {
    if let Some(entry) = registry().lookup(cookie) {
        entry.fail_closed(true);
    }
}

fn fault_cookie_without_unwind(cookie: CallbackCookie) {
    if let Err(payload) = catch_unwind(AssertUnwindSafe(|| fault_cookie(cookie))) {
        std::mem::forget(payload);
    }
}

fn increment_saturating(counter: &AtomicU64) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_add(1))
    });
}

#[cfg(test)]
pub(crate) fn invoke_image_for_test(
    cookie: CallbackCookie,
    image: *mut bindings::MV3D_LP_IMAGE_DATA,
) {
    // SAFETY: tests provide a descriptor whose backing storage remains live through this
    // synchronous invocation, or a stale cookie that must be rejected before payload access.
    unsafe { image_trampoline(image, cookie.as_user_pointer()) }
}

#[cfg(test)]
pub(crate) fn invoke_exception_for_test(
    cookie: CallbackCookie,
    exception: *mut bindings::MV3D_LP_EXCEPTION_INFO,
) {
    // SAFETY: tests provide a live exception descriptor for this synchronous call.
    unsafe { exception_trampoline(exception, cookie.as_user_pointer()) }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{
        CallbackDelivery, CallbackEntry, CallbackRegistry, CallbackSink, FrameCallbackSink,
        MAX_COOKIE,
    };

    fn entry() -> Arc<CallbackEntry> {
        let sink: FrameCallbackSink = Arc::new(|_| CallbackDelivery::Delivered);
        Arc::new(CallbackEntry::new(CallbackSink::Image(sink)))
    }

    // 验证 callback cookie 始终非零且注销后不复用，防止迟到 callback 命中错误注册。
    #[test]
    fn cookies_are_nonzero_and_never_reused_after_removal() {
        let registry = CallbackRegistry::new();
        let first_entry = entry();
        let first = registry.insert(Arc::clone(&first_entry)).unwrap();
        registry.remove(first, &first_entry);
        let second = registry.insert(entry()).unwrap();

        assert_ne!(first, second);
        assert_ne!(first.get(), 0);
        assert_ne!(second.get(), 0);
        assert_eq!(
            super::CallbackCookie::from_user_pointer(second.as_user_pointer()),
            Some(second)
        );
    }

    // 验证 cookie 序列耗尽时终止分配，防止整数回绕重新产生旧 cookie。
    #[test]
    fn cookie_sequence_exhaustion_never_wraps_to_zero() {
        let registry = CallbackRegistry::new();
        registry.lock().next_cookie = MAX_COOKIE;

        let last = registry.insert(entry()).unwrap();
        assert_eq!(last.get(), MAX_COOKIE);
        assert!(registry.insert(entry()).is_none());
    }
}
