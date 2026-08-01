use std::cell::Cell;
use std::fmt;
use std::marker::PhantomData;
use std::time::Duration;

use crate::{Device, Error, Result};

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileTransferDirection {
    DeviceToHost,
    HostToDevice,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileProgress {
    pub completed: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileTransferStatus {
    Running(FileProgress),
    Completed(FileProgress),
}

/// An asynchronous device file transfer that owns its device.
///
/// Unique ownership of the active transfer may move between threads. The transfer remains
/// `!Sync`, so polling and cleanup cannot run concurrently. Dropping this value closes the owned
/// device. Once progress is complete, [`FileTransfer::try_into_device`] returns the device for
/// another operation.
#[must_use = "dropping FileTransfer closes its owned device"]
pub struct FileTransfer<'sdk> {
    inner: mv3d_lp_internal::FileTransfer<'sdk>,
    _not_sync: PhantomData<Cell<()>>,
}

impl<'sdk> FileTransfer<'sdk> {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::FileTransfer<'sdk>) -> Self {
        Self {
            inner,
            _not_sync: PhantomData,
        }
    }

    #[must_use]
    pub fn direction(&self) -> FileTransferDirection {
        match self.inner.direction() {
            mv3d_lp_internal::FileTransferDirection::DeviceToHost => {
                FileTransferDirection::DeviceToHost
            }
            mv3d_lp_internal::FileTransferDirection::HostToDevice => {
                FileTransferDirection::HostToDevice
            }
        }
    }

    pub fn progress(&mut self) -> Result<FileTransferStatus> {
        self.inner
            .progress()
            .map(status_from_internal)
            .map_err(Error::from)
    }

    /// Polls until completion or until `timeout` elapses.
    ///
    /// `Ok(None)` means the timeout elapsed while the transfer was still running. Completed
    /// transfers return their cached final progress without another SDK call, while faulted
    /// transfers return immediately.
    pub fn wait_timeout(
        &mut self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<Option<FileProgress>> {
        self.inner
            .wait_timeout(poll_interval, timeout)
            .map(|progress| progress.map(progress_from_internal))
            .map_err(Error::from)
    }

    /// Returns the owned device only after completion.
    #[allow(
        clippy::result_large_err,
        reason = "the ownership-preserving API must return the unchanged active transfer"
    )]
    pub fn try_into_device(self) -> std::result::Result<(Device<'sdk>, FileProgress), Self> {
        match self.inner.try_into_device() {
            Ok((device, progress)) => Ok((
                Device::from_internal(device),
                progress_from_internal(progress),
            )),
            Err(inner) => Err(Self::from_internal(inner)),
        }
    }

    /// Closes the owned device and reports any cleanup error.
    pub fn close(self) -> Result<()> {
        self.inner.close().map_err(Error::from)
    }
}

fn progress_from_internal(progress: mv3d_lp_internal::FileProgress) -> FileProgress {
    FileProgress {
        completed: progress.completed,
        total: progress.total,
    }
}

fn status_from_internal(status: mv3d_lp_internal::FileTransferStatus) -> FileTransferStatus {
    match status {
        mv3d_lp_internal::FileTransferStatus::Running(progress) => {
            FileTransferStatus::Running(progress_from_internal(progress))
        }
        mv3d_lp_internal::FileTransferStatus::Completed(progress) => {
            FileTransferStatus::Completed(progress_from_internal(progress))
        }
    }
}

enum FileTransferStartErrorKind<'sdk> {
    RejectedBeforeDriverEntry {
        source: Error,
        device: Device<'sdk>,
    },
    FailedAfterDriverEntry {
        start: Error,
        cleanup: Option<Error>,
    },
}

/// A resource-owning error returned when a file transfer cannot be started.
///
/// Validation, state, or runtime rejection before the native Driver is entered retains the
/// device. Consume this error with [`FileTransferStartError::into_rejected_device`] to recover it;
/// dropping the error closes it. A failure after Driver entry closes the device immediately and
/// cannot return it.
#[must_use = "a rejected file transfer may contain a recoverable device"]
pub struct FileTransferStartError<'sdk> {
    kind: Box<FileTransferStartErrorKind<'sdk>>,
}

impl<'sdk> FileTransferStartError<'sdk> {
    pub(crate) fn from_internal(error: mv3d_lp_internal::FileTransferStartError<'sdk>) -> Self {
        let error = match error.into_rejected_device() {
            Ok((source, device)) => {
                return Self {
                    kind: Box::new(FileTransferStartErrorKind::RejectedBeforeDriverEntry {
                        source: Error::from(source),
                        device: Device::from_internal(device),
                    }),
                };
            }
            Err(error) => error,
        };
        match error.into_failed_after_driver_entry() {
            Ok((start, cleanup)) => Self {
                kind: Box::new(FileTransferStartErrorKind::FailedAfterDriverEntry {
                    start: Error::from(start),
                    cleanup: cleanup.map(Error::from),
                }),
            },
            Err(_) => unreachable!("the rejected variant was handled above"),
        }
    }

    /// Returns the validation, state, runtime, or SDK error that prevented the start.
    #[must_use]
    pub fn start_error(&self) -> &Error {
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, .. } => source,
            FileTransferStartErrorKind::FailedAfterDriverEntry { start, .. } => start,
        }
    }

    /// Returns a cleanup error when a post-entry start failure could not close cleanly.
    #[must_use]
    pub fn cleanup_error(&self) -> Option<&Error> {
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { .. } => None,
            FileTransferStartErrorKind::FailedAfterDriverEntry { cleanup, .. } => cleanup.as_ref(),
        }
    }

    /// Recovers the device when the transfer was rejected before entering the native Driver.
    pub fn into_rejected_device(self) -> std::result::Result<(Error, Device<'sdk>), Self> {
        match *self.kind {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, device } => {
                Ok((source, device))
            }
            kind @ FileTransferStartErrorKind::FailedAfterDriverEntry { .. } => Err(Self {
                kind: Box::new(kind),
            }),
        }
    }
}

impl fmt::Debug for FileTransferStartError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = formatter.debug_struct("FileTransferStartError");
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, .. } => debug
                .field("phase", &"rejected before Driver entry")
                .field("source", source),
            FileTransferStartErrorKind::FailedAfterDriverEntry { start, cleanup } => debug
                .field("phase", &"failed after Driver entry")
                .field("start", start)
                .field("cleanup", cleanup),
        };
        debug.finish()
    }
}

impl fmt::Display for FileTransferStartError<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind.as_ref() {
            FileTransferStartErrorKind::RejectedBeforeDriverEntry { source, .. } => write!(
                formatter,
                "file transfer was rejected before entering the Driver: {source}"
            ),
            FileTransferStartErrorKind::FailedAfterDriverEntry {
                start,
                cleanup: Some(cleanup),
            } => write!(
                formatter,
                "file transfer failed after entering the Driver ({start}); device cleanup also failed ({cleanup})"
            ),
            FileTransferStartErrorKind::FailedAfterDriverEntry {
                start,
                cleanup: None,
            } => write!(
                formatter,
                "file transfer failed after entering the Driver and the device was closed: {start}"
            ),
        }
    }
}

impl std::error::Error for FileTransferStartError<'_> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.start_error())
    }
}
