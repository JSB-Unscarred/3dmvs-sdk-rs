#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRange {
    Auto,
    Manual { minimum: i32, maximum: i32 },
}
