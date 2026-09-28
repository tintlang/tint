use crate::Position;

/// A source range bounded by two positions.
///
/// Spans are normalized on construction so `start.offset <= end.offset`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Span {
    pub start: Position,
    pub end: Position,
}

impl Span {
    /// Creates a normalized span, swapping the endpoints when necessary.
    pub fn new(start: Position, end: Position) -> Self {
        if end.offset < start.offset {
            Self {
                start: end,
                end: start,
            }
        } else {
            Self { start, end }
        }
    }

    /// Returns a span with no associated source location.
    pub fn dummy() -> Self {
        Self {
            start: Position::dummy(),
            end: Position::dummy(),
        }
    }

    /// Returns the smallest span that contains both input spans.
    pub fn merge(a: Span, b: Span) -> Span {
        let start = if a.start.offset <= b.start.offset {
            a.start
        } else {
            b.start
        };
        let end = if a.end.offset >= b.end.offset {
            a.end
        } else {
            b.end
        };
        Span { start, end }
    }

    /// Expands this span so that it also contains `other`.
    pub fn extend(&mut self, other: Span) {
        *self = Span::merge(*self, other);
    }

    /// Returns whether `pos` lies inside the span, including both endpoints.
    pub fn contains(&self, pos: Position) -> bool {
        self.start.offset <= pos.offset && pos.offset <= self.end.offset
    }

    /// Returns the span length in UTF-8 bytes.
    pub fn len(&self) -> usize {
        self.end.offset - self.start.offset
    }

    /// Returns whether the span endpoints are ordered by byte offset.
    pub fn is_valid(&self) -> bool {
        self.start.offset <= self.end.offset
    }
}
