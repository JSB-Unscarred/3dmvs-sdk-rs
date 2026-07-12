#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRangeRecord {
    Auto,
    Manual { minimum: i32, maximum: i32 },
}
