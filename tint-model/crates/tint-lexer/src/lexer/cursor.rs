use super::Lexer;
use crate::{Token, TokenKind};
use tint_ast::{Position, Span};

impl Lexer<'_> {
    pub(super) fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;

        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        self.pos += c.len_utf8();
        Some(c)
    }

    pub(super) fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    pub(super) fn peek2(&self) -> Option<(char, char)> {
        let mut chars = self.chars.clone();
        Some((chars.next()?, chars.next()?))
    }

    pub(super) fn peek3(&self) -> Option<(char, char, char)> {
        let mut chars = self.chars.clone();
        Some((chars.next()?, chars.next()?, chars.next()?))
    }

    pub(super) fn position(&self) -> Position {
        Position::new(self.pos, self.line, self.column)
    }

    pub(super) fn single(&mut self, kind: TokenKind) -> Token {
        let start = self.position();
        let c = self.bump().expect("single token requires one character");
        Token::new(kind, Span::new(start, self.position()), c.to_string())
    }

    pub(super) fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.peek().is_some_and(char::is_whitespace) {
                self.bump();
            }

            if self.peek2() != Some(('/', '/')) {
                return;
            }

            self.bump();
            self.bump();

            while let Some(c) = self.peek() {
                if c == '\n' {
                    break;
                }
                self.bump();
            }
        }
    }
}
