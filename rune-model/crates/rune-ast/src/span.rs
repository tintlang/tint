use crate::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        // гарантируем корректность
        if end.offset < start.offset {
            Self { start: end, end: start }
        } else {
            Self { start, end }
        }
    }

    pub fn dummy() -> Self {
        Self {
            start: Position::dummy(),
            end: Position::dummy(),
        }
    }

    /// Объединяет два span независимо от их порядка
    pub fn merge(a: Span, b: Span) -> Span {
        let start = if a.start.offset <= b.start.offset { a.start } else { b.start };
        let end = if a.end.offset >= b.end.offset { a.end } else { b.end };
        Span { start, end }
    }

    /// Расширяет this span включением другого
    pub fn extend(&mut self, other: Span) {
        *self = Span::merge(*self, other);
    }

    pub fn contains(&self, pos: Position) -> bool {
        self.start.offset <= pos.offset && pos.offset <= self.end.offset
    }

    pub fn len(&self) -> usize {
        (self.end.offset - self.start.offset) as usize
    }

    pub fn is_valid(&self) -> bool {
        self.start.offset <= self.end.offset
    }
}
