use std::fmt;
use std::io;
use std::num::NonZeroUsize;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::mpsc::Receiver;
use std::thread::{self, JoinHandle};

use crate::SdkText;

/// Configuration shared by the safe callback receiver APIs.
///
/// Callback queues are bounded so that a stalled consumer cannot retain an
/// unbounded number of owned frame buffers. Native callback trampolines must
/// use non-blocking sends and discard a new event when this queue is full;
/// they must never wait for the Rust consumer while running on an SDK thread.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct CallbackOptions {
    /// Maximum number of owned events waiting for the receiver.
    ///
    /// Registration rejects values above [`CallbackOptions::MAX_QUEUE_CAPACITY`]
    /// before allocating a channel.
    pub queue_capacity: NonZeroUsize,
}

/// A snapshot of callback delivery and validation counters.
#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct CallbackStats {
    pub delivered: u64,
    pub dropped_full: u64,
    pub invalid_payloads: u64,
    pub panics: u64,
    pub accepting: bool,
}

impl CallbackStats {
    pub(crate) fn from_internal(record: mv3d_lp_internal::CallbackStatsRecord) -> Self {
        Self {
            delivered: record.delivered,
            dropped_full: record.dropped_full,
            invalid_payloads: record.invalid_payloads,
            panics: record.panics,
            accepting: record.accepting,
        }
    }
}

impl CallbackOptions {
    /// Largest queue accepted by the callback registration APIs.
    pub const MAX_QUEUE_CAPACITY: usize = 64;
    pub const DEFAULT_QUEUE_CAPACITY: NonZeroUsize =
        NonZeroUsize::new(4).expect("the default callback queue capacity is non-zero");

    #[must_use]
    pub const fn new(queue_capacity: NonZeroUsize) -> Self {
        Self { queue_capacity }
    }
}

impl Default for CallbackOptions {
    fn default() -> Self {
        Self::new(Self::DEFAULT_QUEUE_CAPACITY)
    }
}

/// A device exception type reported by the SDK, preserving unknown values.
///
/// This is a newtype rather than a Rust enum so exceptions introduced by a
/// newer runtime remain representable.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct DeviceExceptionType(u32);

impl DeviceExceptionType {
    pub const UNDEFINED: Self = Self(0xFFFF_FFFF);
    pub const DISCONNECTED: Self = Self(0x0000_0001);

    #[must_use]
    pub const fn from_raw(raw: i32) -> Self {
        Self(raw as u32)
    }

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self(bits)
    }

    #[must_use]
    pub const fn raw(self) -> i32 {
        self.0 as i32
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn name(self) -> Option<&'static str> {
        match self.0 {
            0xFFFF_FFFF => Some("undefined"),
            0x0000_0001 => Some("disconnected"),
            _ => None,
        }
    }
}

impl fmt::Debug for DeviceExceptionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => write!(formatter, "DeviceExceptionType({name}, 0x{:08X})", self.0),
            None => write!(formatter, "DeviceExceptionType(0x{:08X})", self.0),
        }
    }
}

impl fmt::Display for DeviceExceptionType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.name() {
            Some(name) => formatter.write_str(name),
            None => write!(formatter, "unknown device exception 0x{:08X}", self.0),
        }
    }
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

/// The terminal state observed when joining a [`CallbackWorker`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum CallbackWorkerExit {
    /// Every sender was dropped and the worker consumed all queued events.
    ChannelClosed,
    /// The user-provided handler panicked. The unwind was contained inside the
    /// Rust worker thread and never reached an SDK callback trampoline.
    HandlerPanicked,
    /// The worker terminated outside its handler panic boundary.
    WorkerPanicked,
}

/// A Rust thread that serially consumes owned callback events.
///
/// User handlers execute only on this worker, never on an SDK callback thread.
/// The worker catches unwinding panics from the handler and reports them from
/// [`CallbackWorker::join`]. With `panic = "abort"`, Rust cannot isolate a
/// handler panic.
///
/// Dropping this value detaches the thread. The detached thread owns only its
/// receiver, handler, and already-owned events; it does not retain a device,
/// SDK handle, or native callback payload.
#[must_use = "dropping CallbackWorker detaches its Rust worker thread"]
pub struct CallbackWorker {
    handle: Option<JoinHandle<CallbackWorkerExit>>,
}

impl CallbackWorker {
    /// Spawns a named Rust thread that invokes `handler` serially for events
    /// received from `receiver`.
    pub fn spawn<T, F>(receiver: Receiver<T>, mut handler: F) -> io::Result<Self>
    where
        T: Send + 'static,
        F: FnMut(T) + Send + 'static,
    {
        let handle = thread::Builder::new()
            .name("mv3d-lp-callback".to_owned())
            .spawn(move || {
                let result = catch_unwind(AssertUnwindSafe(|| {
                    while let Ok(event) = receiver.recv() {
                        handler(event);
                    }
                }));
                match result {
                    Ok(()) => CallbackWorkerExit::ChannelClosed,
                    Err(_) => CallbackWorkerExit::HandlerPanicked,
                }
            })?;
        Ok(Self {
            handle: Some(handle),
        })
    }

    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.handle
            .as_ref()
            .is_none_or(std::thread::JoinHandle::is_finished)
    }

    /// Waits for the channel to close or the handler to panic.
    ///
    /// This method should not be called while holding an SDK call lock. It can
    /// wait indefinitely when a user handler does not return.
    ///
    /// # Panics
    ///
    /// This method may panic if dropping a user-defined thread panic payload
    /// itself panics. Panic payloads follow normal Rust drop semantics.
    #[must_use]
    pub fn join(mut self) -> CallbackWorkerExit {
        let Some(handle) = self.handle.take() else {
            return CallbackWorkerExit::WorkerPanicked;
        };
        match handle.join() {
            Ok(exit) => exit,
            Err(_) => CallbackWorkerExit::WorkerPanicked,
        }
    }
}

impl fmt::Debug for CallbackWorker {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallbackWorker")
            .field("is_finished", &self.is_finished())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;
    use std::panic::panic_any;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, mpsc};
    use std::thread;

    use crate::SdkText;

    use super::{
        CallbackOptions, CallbackWorker, CallbackWorkerExit, DeviceException, DeviceExceptionType,
    };

    struct DropProbe(Arc<AtomicUsize>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn callback_options_default_to_a_bounded_non_zero_queue() {
        assert_eq!(CallbackOptions::default().queue_capacity.get(), 4);
        let options = CallbackOptions::new(NonZeroUsize::new(2).unwrap());
        let (sender, receiver) = mpsc::sync_channel(options.queue_capacity.get());
        sender.try_send(1).unwrap();
        sender.try_send(2).unwrap();
        assert!(sender.try_send(3).is_err());
        assert_eq!(receiver.recv().unwrap(), 1);
        assert_eq!(receiver.recv().unwrap(), 2);
    }

    #[test]
    fn device_exception_types_preserve_unknown_bits() {
        assert_eq!(DeviceExceptionType::UNDEFINED.raw(), -1);
        assert_eq!(DeviceExceptionType::DISCONNECTED.bits(), 1);

        let unknown = DeviceExceptionType::from_bits(0xDEAD_BEEF);
        assert_eq!(unknown.bits(), 0xDEAD_BEEF);
        assert_eq!(unknown.raw(), 0xDEAD_BEEF_u32 as i32);
        assert_eq!(unknown.name(), None);
        assert!(format!("{unknown:?}").contains("0xDEADBEEF"));
    }

    #[test]
    fn device_exceptions_own_their_description() {
        let event = DeviceException::new(
            DeviceExceptionType::DISCONNECTED,
            SdkText::new(b"link lost").unwrap(),
        );
        assert_eq!(event.kind, DeviceExceptionType::DISCONNECTED);
        assert_eq!(event.description.as_bytes(), b"link lost");
    }

    #[test]
    fn worker_invokes_handler_on_its_rust_thread() {
        let caller = thread::current().id();
        let (event_sender, event_receiver) = mpsc::channel();
        let (result_sender, result_receiver) = mpsc::channel();
        let worker = CallbackWorker::spawn(event_receiver, move |value| {
            result_sender.send((thread::current().id(), value)).unwrap();
        })
        .unwrap();

        event_sender.send(37).unwrap();
        drop(event_sender);

        let (worker_thread, value) = result_receiver.recv().unwrap();
        assert_ne!(worker_thread, caller);
        assert_eq!(value, 37);
        assert_eq!(worker.join(), CallbackWorkerExit::ChannelClosed);
    }

    #[test]
    fn worker_contains_handler_panics() {
        let (sender, receiver) = mpsc::channel();
        let worker = CallbackWorker::spawn(receiver, |_: ()| {
            panic!("test handler panic");
        })
        .unwrap();
        sender.send(()).unwrap();

        assert_eq!(worker.join(), CallbackWorkerExit::HandlerPanicked);
    }

    #[test]
    fn worker_drops_handler_panic_payload() {
        let drops = Arc::new(AtomicUsize::new(0));
        let handler_drops = Arc::clone(&drops);
        let (sender, receiver) = mpsc::channel();
        let worker = CallbackWorker::spawn(receiver, move |_: ()| {
            panic_any(DropProbe(Arc::clone(&handler_drops)));
        })
        .unwrap();
        sender.send(()).unwrap();

        assert_eq!(worker.join(), CallbackWorkerExit::HandlerPanicked);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn join_drops_worker_panic_payload() {
        struct PanicWithPayloadOnDrop(Arc<AtomicUsize>);

        impl Drop for PanicWithPayloadOnDrop {
            fn drop(&mut self) {
                panic_any(DropProbe(Arc::clone(&self.0)));
            }
        }

        let drops = Arc::new(AtomicUsize::new(0));
        let captured = PanicWithPayloadOnDrop(Arc::clone(&drops));
        let (sender, receiver) = mpsc::channel::<()>();
        let worker = CallbackWorker::spawn(receiver, move |_| {
            let _keep_capture_alive = &captured;
        })
        .unwrap();
        drop(sender);

        assert_eq!(worker.join(), CallbackWorkerExit::WorkerPanicked);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
