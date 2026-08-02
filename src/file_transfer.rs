use std::time::Duration;

use crate::{Error, Result};

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

/// An asynchronous file transfer borrowing its device exclusively.
///
/// Unique ownership of the guard may move between threads. It remains `!Sync`, so polling cannot
/// run concurrently. Dropping the guard does not cancel the transfer; use
/// [`crate::Device::active_file_transfer`] to resume polling it.
#[must_use = "dropping FileTransfer does not cancel the device transfer"]
pub struct FileTransfer<'device> {
    inner: mv3d_lp_internal::FileTransfer<'device>,
}

impl<'device> FileTransfer<'device> {
    pub(crate) fn from_internal(inner: mv3d_lp_internal::FileTransfer<'device>) -> Self {
        Self { inner }
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
    /// `Ok(None)` means the timeout elapsed while the transfer was still running. A polling error
    /// ends only the current call, so callers may retry.
    ///
    /// Each successful progress snapshot is validated independently. The SDK does not promise
    /// that `completed` is monotonic or that `total` remains fixed between calls.
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
