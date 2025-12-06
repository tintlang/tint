#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub struct Position {
    // UTF-8 offset от начала файла (в байтах)
    pub offset: usize,
    // (1-based)
    pub line: usize,
    // (1-based, UTF-8 aware)
    pub column: usize,
}

impl Position {
    pub fn new(offset: usize, line: usize, column: usize) -> Self {
        Self { offset, line, column }
    }

    // Позиция-заглушка (например, для ошибок без локации)
    pub fn dummy() -> Self {
        Position { offset: 0, line: 0, column: 0 }
    }

    // Смещение по байтам (используется lexer'ом)
    pub fn advance_byte(&mut self, byte_len: usize) {
        self.offset += byte_len;
        self.column += 1; // колонка увеличивается на символ
    }

    // Переход на новую строку
    pub fn new_line(&mut self) {
        self.line += 1;
        self.column = 1;
    }

    // Проверка dummy-позиции
    pub fn is_dummy(&self) -> bool {
        self.line == 0 && self.column == 0
    }

    // Возвращает позицию после текущей, с учётом символа
    pub fn advanced(&self, byte_len: usize) -> Position {
        Position {
            offset: self.offset + byte_len,
            line: self.line,
            column: self.column + 1,
        }
    }

    // Расстояние между позициями в байтах (если в одной строке)
    pub fn distance_to(&self, other: Position) -> Option<usize> {
        if self.line == other.line {
            Some(other.offset.saturating_sub(self.offset))
        } else {
            None
        }
    }
}
