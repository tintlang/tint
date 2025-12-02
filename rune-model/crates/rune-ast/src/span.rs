use crate::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    pub fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }

    /// Используется, когда span ещё неизвестен (например, при ошибках)
    pub fn dummy() -> Self {
        Span { start: Position::dummy(), end: Position::dummy() }
    }

    /// Объединяет два span в один общий диапазон
    pub fn merge(a: Span, b: Span) -> Span {
        Span {
            start: a.start,
            end: b.end,
        }
    }

    /// Полезно в парсере: расширить текущий span до охвата нового
    pub fn join(&mut self, other: Span) {
        self.end = other.end;
    }
}
