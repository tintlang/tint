#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Mode {
    #[default]
    Logic,
    UI,
}

#[derive(Default)]
pub struct CheckerContext {
    pub mode: Mode,
    pub inside_borrow: bool,
    /// Report every expression whose type could not be inferred
    /// (`CannotInfer`) instead of leaving it `Unknown`.
    pub strict: bool,
    /// Names of host functions (Rust/JS natives) the program may call.
    pub host_fns: Vec<String>,
}
