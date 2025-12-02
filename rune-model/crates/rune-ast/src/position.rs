
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Абсолютный offset (байты от начала файла)
    pub offset: usize,

    /// Номер строки (1-based)
    pub line: usize,

    /// Номер колонки (1-based, учитывает UTF-8)
    pub column: usize,
}

impl Position {
    pub fn new(offset: usize, line: usize, column: usize) -> Self {
        Self { offset, line, column }
    }

    pub fn dummy() -> Self {
        Position { offset: 0, line: 0, column: 0 }
    }
}