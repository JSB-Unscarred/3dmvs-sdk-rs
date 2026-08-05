#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileProgressRaw {
    pub completed: i64,
    pub total: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileProgress {
    pub completed: u64,
    pub total: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FileTransferStatus {
    Running(FileProgress),
    Completed(FileProgress),
}
