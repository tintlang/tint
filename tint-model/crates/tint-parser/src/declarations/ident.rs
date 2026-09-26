use crate::{error::*, Parser};
use tint_lexer::{Token, TokenKind};

impl Parser {
    /// Parse identifier (variable names, component names, module segments)
    /// Forbids reserved keywords.
    pub fn parse_ident(&mut self) -> PResult<String> {
        let tok: Token = self.stream.next_owned();

        match tok.kind {
            TokenKind::Ident | TokenKind::SelfKw => {
                return Ok(tok.lexeme.clone());
            }

            // DISALLOWED KEYWORDS:
            TokenKind::Fn
            | TokenKind::Let
            | TokenKind::Return
            | TokenKind::If
            | TokenKind::Else
            | TokenKind::Match
            | TokenKind::For
            | TokenKind::While
            | TokenKind::Loop
            | TokenKind::Break
            | TokenKind::Continue
            | TokenKind::Async
            | TokenKind::Await
            | TokenKind::Try
            | TokenKind::Struct
            | TokenKind::Enum
            | TokenKind::Impl
            | TokenKind::Borrow
            | TokenKind::Immut
            | TokenKind::Move
            | TokenKind::Clone
            | TokenKind::Ui
            | TokenKind::State
            | TokenKind::Signal
            | TokenKind::Computed
            | TokenKind::Module
            | TokenKind::Export
            | TokenKind::Use
            | TokenKind::Tint2d
            | TokenKind::In
            | TokenKind::Where => {
                return Err(ParserError::Message {
                    msg: format!("Keyword '{}' cannot be used as identifier", tok.lexeme),
                    span: tok.span,
                });
            }

            _ => {
                return Err(ParserError::Message {
                    msg: format!("Expected identifier, found {:?}", tok.kind),
                    span: tok.span,
                });
            }
        }
    }
}
