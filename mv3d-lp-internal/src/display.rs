/// 渲染到窗口时使用的深度取值范围。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DisplayRange {
    /// 由 SDK 自动确定范围。
    Auto,
    /// 由调用方指定范围。
    Manual {
        /// 范围下界。
        minimum: i32,
        /// 范围上界。
        maximum: i32,
    },
}
