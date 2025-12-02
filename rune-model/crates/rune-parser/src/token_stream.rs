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

    /// Peek current token
    pub fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap()
    }

    /// Peek N tokens ahead
    pub fn peek_n(&self, n: usize) -> &Token {
        self.tokens.get(self.pos + n).unwrap()
    }

    /// True if EOF reached
    pub fn is_eof(&self) -> bool {
        self.peek().kind == TokenKind::Eof
    }

    /// Take next token by reference (advance)
    pub fn next(&mut self) -> Token {
        let t = self.tokens[self.pos].clone();
        self.pos += 1;
        t
    }

    /// Take next token by value (clone)
    pub fn next_owned(&mut self) -> Token {
        let t = self.peek().clone();
        self.pos += 1;
        t
    }

    /// Require token of kind
    pub fn expect(&mut self, kind: TokenKind) -> Result<Token, ParserError> {
        let token = self.peek().clone();

        if token.kind == kind {
            self.pos += 1;
            Ok(token)
        } else {
            Err(ParserError::Unexpected {
                expected: kind,
                found: token.kind,
                span: token.span,
            })
        }
    }

    /// Span of the previous token (pos - 1)
    pub fn last_span(&self) -> Span {
        if self.pos == 0 {
            self.tokens[0].span
        } else {
            self.tokens[self.pos - 1].span
        }
    }

    /// If next token matches → consume + return true
    pub fn consume_if(&mut self, kind: TokenKind) -> bool {
        if self.peek().kind == kind {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    /// Same but returns the token (Option<Token>)
    pub fn consume_if_ret(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek().kind == kind {
            let t = self.peek().clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }
}
