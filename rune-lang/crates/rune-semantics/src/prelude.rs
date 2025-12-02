#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Mode {
    Logic,
    UI,
}

#[derive(Default)]
pub struct CheckerContext {
    pub mode: Mode,
    pub inside_borrow: bool,
}
