/// Signed file-transfer counters copied without inferring completion semantics.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct FileProgress {
    /// Value reported by `nCompleted`.
    pub completed: i64,
    /// Value reported by `nTotal`.
    pub total: i64,
}
