mod cursor;
mod literals;
mod operators;

use crate::{Token, TokenKind};
use std::str::Chars;
use tint_ast::Span;

pub struct Lexer<'a> {
    pub(super) chars: Chars<'a>,
    pub(super) pos: usize,
    pub(super) line: usize,
    pub(super) column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            chars: src.chars(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        let start = self.position();
        let Some(c) = self.peek() else {
            return Token::new(TokenKind::Eof, Span::new(start, start), "");
        };

        if let Some(token) = self.lex_operator(start) {
            return token;
        }

        match c {
            '{' => self.single(TokenKind::LBrace),
            '}' => self.single(TokenKind::RBrace),
            '(' => self.single(TokenKind::LParen),
            ')' => self.single(TokenKind::RParen),
            '<' => self.single(TokenKind::LAngle),
            '>' => self.single(TokenKind::RAngle),
            '[' => self.single(TokenKind::LBracket),
            ']' => self.single(TokenKind::RBracket),
            ',' => self.single(TokenKind::Comma),
            ':' => self.single(TokenKind::Colon),
            ';' => self.single(TokenKind::Semicolon),
            '.' => self.single(TokenKind::Dot),
            '+' => self.single(TokenKind::Plus),
            '-' => self.single(TokenKind::Minus),
            '*' => self.single(TokenKind::Star),
            '/' => self.single(TokenKind::Slash),
            '%' => self.single(TokenKind::Percent),
            '!' => self.single(TokenKind::Bang),
            '?' => self.single(TokenKind::Question),
            '|' => self.single(TokenKind::Pipe),
            '=' => self.single(TokenKind::Eq),
            '@' => self.single(TokenKind::At),
            '"' => self.lex_string(start),
            '#' => self.lex_color(start),
            c if c.is_ascii_digit() => self.lex_number(start),
            c if is_ident_start(c) => self.lex_ident(start),
            _ => {
                // Preserve unknown input as an identifier-like token so parsing can report it later.
                self.bump();
                Token::new(
                    TokenKind::Ident,
                    Span::new(start, self.position()),
                    c.to_string(),
                )
            }
        }
    }
}

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

pub(super) fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
