use crate::error::{ParserError, PResult};
use rune_ast::Span;
use rune_lexer::{Token, TokenKind};

#[derive(Clone)]
pub struct TokenStream {
    tokens: Vec<Token>,
    pos: usize,
}

impl TokenStream {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn clone_with_reset(&self) -> Self {
        Self { tokens: self.tokens.clone(), pos: 0 }
    }

    pub fn peek(&self) -> &Token {
        self.tokens
            .get(self.pos)
            .unwrap_or_else(|| self.tokens.last().expect("token stream cannot be empty"))
    }

    pub fn next(&mut self) -> Token {
        let token = self.peek().clone();
        if !self.at_end() {
            self.pos += 1;
        }
        token
    }

    pub fn checkpoint(&self) -> usize { self.pos }

    pub fn restore(&mut self, checkpoint: usize) { self.pos = checkpoint; }

    pub fn consume_if(&mut self, kind: TokenKind) -> bool {
        if self.check(kind) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    pub fn consume_if_ret(&mut self, kind: TokenKind) -> Option<Token> {
        if self.check(kind) {
            let token = self.peek().clone();
            self.pos += 1;
            Some(token)
        } else {
            None
        }
    }

    pub fn expect(&mut self, kind: TokenKind) -> PResult<Token> {
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

    pub fn peek_n(&self, n: usize) -> &Token {
        self.tokens
            .get(self.pos + n)
            .unwrap_or_else(|| self.tokens.last().expect("token stream cannot be empty"))
    }

    pub fn peek_n_kind(&self, n: usize) -> TokenKind { self.peek_n(n).kind.clone() }

    pub fn prev(&self) -> &Token {
        if self.pos == 0 { &self.tokens[0] } else { &self.tokens[self.pos - 1] }
    }

    pub fn at_end(&self) -> bool { self.peek().kind == TokenKind::Eof }

    pub fn check(&self, kind: TokenKind) -> bool { self.peek().kind == kind }

    pub fn check2(&self, first: TokenKind, second: TokenKind) -> bool {
        self.check(first) && self.peek_n(1).kind == second
    }

    pub fn peek_kind(&self) -> TokenKind { self.peek().kind.clone() }

    pub fn peek2_kind(&self) -> TokenKind { self.peek_n(1).kind.clone() }

    pub fn next_owned(&mut self) -> Token { self.next() }

    pub fn error_here<T>(&self, msg: impl Into<String>) -> PResult<T> {
        Err(ParserError::Message { msg: msg.into(), span: self.peek().span })
    }

    pub fn last_span(&self) -> Span { self.prev().span }

    pub fn expect_ident(&mut self) -> PResult<Token> { self.expect(TokenKind::Ident) }

    pub fn expect_string(&mut self) -> PResult<Token> { self.expect(TokenKind::String) }

    /// Returns true for `name[.segment]* {`, the prefix used by block-style UI modifiers.
    pub fn is_modifier_start(&self) -> bool {
        if self.peek().kind != TokenKind::Ident {
            return false;
        }

        let mut offset = 1;
        while self.peek_n(offset - 1).kind == TokenKind::Ident
            && self.peek_n(offset).kind == TokenKind::Dot
            && self.peek_n(offset + 1).kind == TokenKind::Ident
        {
            offset += 2;
        }

        self.peek_n(offset).kind == TokenKind::LBrace
    }
}
