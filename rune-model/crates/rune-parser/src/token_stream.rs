// rune-parser/token_stream.rs

use rune_lexer::{Token, TokenKind};
use crate::error::*;
use rune_ast::Span;

pub struct TokenStream {
    tokens: Vec<Token>,
    pos: usize,
}

impl TokenStream {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    // BASIC ACCESS
    /// Peek current token safely
    pub fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or_else(|| {
            self.tokens.last().expect("token stream can't be empty")
        })
    }

    /// Peek token N positions ahead safely
    pub fn peek_n(&self, n: usize) -> &Token {
        self.tokens.get(self.pos + n).unwrap_or_else(|| {
            self.tokens.last().expect("token stream can't be empty")
        })
    }

    /// Return previous token (or first)
    pub fn prev(&self) -> &Token {
        if self.pos == 0 {
            &self.tokens[0]
        } else {
            &self.tokens[self.pos - 1]
        }
    }

    pub fn at_end(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    // ADVANCED CHECKS
    /// Check if current token is of kind
    pub fn check(&self, kind: TokenKind) -> bool {
        self.peek().kind == kind
    }

    /// Check 2-token sequence
    pub fn check2(&self, a: TokenKind, b: TokenKind) -> bool {
        self.peek().kind == a && self.peek_n(1).kind == b
    }

    /// Check token kind without cloning token
    pub fn peek_kind(&self) -> TokenKind {
        self.peek().kind.clone()
    }

    pub fn peek2_kind(&self) -> TokenKind {
        self.peek_n(1).kind.clone()
    }

    // CONSUME
    pub fn next(&mut self) -> Token {
        let t = self.peek().clone();
        if !self.at_end() {
            self.pos += 1;
        }
        t
    }

    pub fn next_owned(&mut self) -> Token {
        self.next()
    }

    /// Consume ONLY if matches
    pub fn consume_if(&mut self, kind: TokenKind) -> bool {
        if self.check(kind.clone()) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub fn consume_if_ret(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind.clone()) {
            let t = self.peek().clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    /// Expect EXACT token or throw error
    pub fn expect(&mut self, kind: TokenKind) -> Result<Token, ParserError> {
        let tok = self.peek().clone();
        if tok.kind == kind {
            self.pos += 1;
            Ok(tok)
        } else {
            Err(ParserError::Unexpected {
                expected: kind,
                found: tok.kind,
                span: tok.span,
            })
        }
    }

    // ERROR HELPERS
    pub fn error_here<T>(&self, msg: impl Into<String>) -> Result<T, ParserError> {
        Err(ParserError::Message {
            msg: msg.into(),
            span: self.peek().span,
        })
    }

    pub fn last_span(&self) -> Span {
        self.prev().span
    }
}
