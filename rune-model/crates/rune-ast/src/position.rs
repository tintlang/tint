/// A source position tracked by byte offset, line, and column.
///
/// `offset` is zero-based and measured in UTF-8 bytes. `line` and `column`
/// are one-based for real source locations; `(0, 0)` is reserved for dummy
/// positions when no source location is available.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub struct Position {
    /// Zero-based UTF-8 byte offset from the start of the source file.
    pub offset: usize,
    /// One-based source line.
    pub line: usize,
    /// One-based character column.
    pub column: usize,
}

impl Position {
    pub fn new(offset: usize, line: usize, column: usize) -> Self {
        Self {
            offset,
            line,
            column,
        }
    }

    /// Returns a sentinel position for AST nodes or diagnostics without a location.
    pub fn dummy() -> Self {
        Position {
            offset: 0,
            line: 0,
            column: 0,
        }
    }

    /// Advances the byte offset by `byte_len` and the column by one character.
    ///
    /// The caller provides the UTF-8 width of the consumed character. Newlines
    /// must be handled separately with [`Position::new_line`].
    pub fn advance_byte(&mut self, byte_len: usize) {
        self.offset += byte_len;
        self.column += 1;
    }

    /// Moves the position to the first column of the next source line.
    pub fn new_line(&mut self) {
        self.line += 1;
        self.column = 1;
    }

    /// Returns `true` when this position is the no-location sentinel.
    pub fn is_dummy(&self) -> bool {
        self.line == 0 && self.column == 0
    }

    /// Returns the position immediately after one consumed character.
    pub fn advanced(&self, byte_len: usize) -> Position {
        Position {
            offset: self.offset + byte_len,
            line: self.line,
            column: self.column + 1,
        }
    }

    /// Returns the byte distance to `other` when both positions are on the same line.
    pub fn distance_to(&self, other: Position) -> Option<usize> {
        if self.line == other.line {
            Some(other.offset.saturating_sub(self.offset))
        } else {
            None
        }
    }
}
