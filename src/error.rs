pub use mv3d_lp_internal::{
    ContractViolation, Error, InputViolation, Operation, SdkError, StatusCode,
};

/// 本 crate 全部接口共用的 `Result` 别名。
pub type Result<T> = std::result::Result<T, Error>;
