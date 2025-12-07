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

     pub fn clone_with_reset(&self) -> TokenStream {
        TokenStream {
            tokens: self.tokens.clone(),
            pos: 0,
        }
    }

    /// Peek token safely + log
pub fn peek(&self) -> &Token {
    let t = self.tokens.get(self.pos).unwrap();
    eprintln!("👁️  PEEK  [{}] {:?} '{}' @ {:?}", 
        self.pos, t.kind, t.lexeme, t.span
    );
    t
}


/// Move stream forward + log
pub fn next(&mut self) -> Token {
    let t = self.tokens.get(self.pos).unwrap().clone();
    eprintln!("➡️  NEXT  [{}] {:?} '{}' @ {:?}", 
        self.pos, t.kind, t.lexeme, t.span
    );
    if !self.at_end() {
        self.pos += 1;
    }
    t
}

    /// Save current token index for speculative parsing
    pub fn checkpoint(&self) -> usize {
        self.pos
    }

    /// Restore stream to earlier position
    pub fn restore(&mut self, checkpoint: usize) {
        self.pos = checkpoint;
    }

/// consume_if with log
pub fn consume_if(&mut self, kind: TokenKind) -> bool {
    if self.peek().kind == kind {
        eprintln!("✔️  CONSUME  {:?} at [{}]", kind, self.pos);
        self.pos += 1;
        true
    } else {
        false
    }
}

/// expect with log
pub fn expect(&mut self, kind: TokenKind) -> Result<Token, ParserError> {
    let t = self.peek().clone();

    if t.kind == kind {
        eprintln!("✔️  EXPECT OK: {:?}", kind);
        self.pos += 1;
        Ok(t)
    } else {
        eprintln!("❌ EXPECT FAIL: expected {:?}, found {:?} @ {:?}", 
            kind, t.kind, t.span
        );
        Err(ParserError::Unexpected {
            expected: kind,
            found: t.kind,
            span: t.span,
        })
    }
}




    /// Peek token N positions ahead safely
    pub fn peek_n(&self, n: usize) -> &Token {
        self.tokens.get(self.pos + n).unwrap_or_else(|| {
            self.tokens.last().expect("token stream can't be empty")
        })
    }

      pub fn peek_n_kind(&self, n: usize) -> TokenKind {
        self.peek_n(n).kind.clone()
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



    pub fn next_owned(&mut self) -> Token {
        self.next()
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

      pub fn expect_ident(&mut self) -> Result<Token, ParserError> {
        let tok = self.peek().clone();

        if let TokenKind::Ident = tok.kind {
            self.pos += 1;
            Ok(tok)
        } else {
            Err(ParserError::Unexpected {
                expected: TokenKind::Ident,
                found: tok.kind,
                span: tok.span,
            })
        }
    }

    pub fn expect_string(&mut self) -> Result<Token, ParserError> {
        let tok = self.peek().clone();

        if let TokenKind::String = tok.kind {
            self.pos += 1;
            Ok(tok)
        } else {
            Err(ParserError::Unexpected {
                expected: TokenKind::String,
                found: tok.kind,
                span: tok.span,
            })
        }
    }

        pub fn is_modifier_start(&self) -> bool {
        // must start with Ident
        if self.peek().kind != TokenKind::Ident {
            return false;
        }

        // Look ahead for pattern: ident [ . ident ]* LBrace
        let mut i = 1;

        // Consume chains like "padding.x.y"
        while self.peek_n(i - 1).kind == TokenKind::Ident
            && self.peek_n(i).kind == TokenKind::Dot
            && self.peek_n(i + 1).kind == TokenKind::Ident
        {
            i += 2; // skip ". ident"
        }

        // After the chain → must be '{'
        self.peek_n(i).kind == TokenKind::LBrace
    }

    
    
}

