use std::fmt;
use std::num::NonZeroUsize;

use crate::SdkText;

/// Configuration shared by the safe callback receiver APIs.
///
/// Callback queues are bounded so that a stalled consumer cannot retain an
/// unbounded number of owned event buffers. Native callback trampolines use
/// non-blocking sends: image delivery discards a new frame when this queue is
/// full, while exception delivery discards the current event and stops so the
/// receiver observes a disconnection. The channel does not encode the disconnection reason.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub struct CallbackOptions {
    /// Maximum number of owned events waiting for the receiver.
    ///
    /// Queued events own their data; image events retain copied payloads. Choose
    /// this capacity based on event size, callback rate, and consumer latency.
    pub queue_capacity: NonZeroUsize,
}

impl CallbackOptions {
    /// Default number of events retained while the consumer is delayed.
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
