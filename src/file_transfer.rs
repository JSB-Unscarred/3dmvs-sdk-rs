/// One progress snapshot copied from the active native file transfer.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileProgress {
    pub completed: u64,
    pub total: u64,
}

/// State reported while polling the file transfer owned by [`crate::Device`].
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum FileTransferStatus {
    Running(FileProgress),
    Completed(FileProgress),
}

pub(crate) fn progress_from_internal(progress: mv3d_lp_internal::FileProgress) -> FileProgress {
    FileProgress {
        completed: progress.completed,
        total: progress.total,
    }
}

pub(crate) fn status_from_internal(
    status: mv3d_lp_internal::FileTransferStatus,
) -> FileTransferStatus {
    match status {
        mv3d_lp_internal::FileTransferStatus::Running(progress) => {
            FileTransferStatus::Running(progress_from_internal(progress))
        }
        mv3d_lp_internal::FileTransferStatus::Completed(progress) => {
            FileTransferStatus::Completed(progress_from_internal(progress))
        }
    }
}
