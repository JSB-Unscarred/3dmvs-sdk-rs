use std::marker::PhantomData;
use std::rc::Rc;
use std::time::{Duration, Instant};

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

/// An asynchronous camera file transfer.
///
/// The camera retains both filename allocations even if this guard is dropped.
/// Use [`crate::Camera::active_file_transfer`] to resume polling.
#[must_use = "dropping FileTransfer does not cancel the device transfer"]
pub struct FileTransfer<'camera, 'sdk> {
    pub(crate) inner: mv3d_lp_internal::FileTransfer<'camera, 'sdk>,
    pub(crate) _not_send_or_sync: PhantomData<Rc<()>>,
}

impl FileTransfer<'_, '_> {
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
    /// `Ok(None)` means the timeout elapsed while the transfer was still running.
    pub fn wait_timeout(
        &mut self,
        poll_interval: Duration,
        timeout: Duration,
    ) -> Result<Option<FileProgress>> {
        let started = Instant::now();
        let poll_interval = poll_interval.max(Duration::from_millis(1));
        loop {
            match self.progress()? {
                FileTransferStatus::Completed(progress) => return Ok(Some(progress)),
                FileTransferStatus::Running(_) => {}
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Ok(None);
            }
            let remaining = timeout.saturating_sub(elapsed);
            std::thread::sleep(poll_interval.min(remaining));
        }
    }
}

fn status_from_internal(status: mv3d_lp_internal::FileTransferStatus) -> FileTransferStatus {
    match status {
        mv3d_lp_internal::FileTransferStatus::Running(progress) => {
            FileTransferStatus::Running(FileProgress {
                completed: progress.completed,
                total: progress.total,
            })
        }
        mv3d_lp_internal::FileTransferStatus::Completed(progress) => {
            FileTransferStatus::Completed(FileProgress {
                completed: progress.completed,
                total: progress.total,
            })
        }
    }
}
