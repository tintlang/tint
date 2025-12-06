// rune-parser/skip.rs

use rune_lexer::TokenKind;
use crate::error::PResult;
use crate::Parser;

impl Parser {
    pub(crate) fn skip_fn_decl(&mut self) -> PResult<()> {
        self.skip_parens()?;
        self.skip_block()?;
        Ok(())
    }

    pub(crate) fn skip_struct_decl(&mut self) -> PResult<()> {
        self.skip_generic_params()?;
        self.skip_braces()
    }

    pub(crate) fn skip_enum_decl(&mut self) -> PResult<()> {
        self.skip_generic_params()?;
        self.skip_braces()
    }

    pub(crate) fn skip_parens(&mut self) -> PResult<()> {
        self.stream.expect(TokenKind::LParen)?;
        let mut depth = 1;

        while depth > 0 {
            let tok = self.stream.next();
            match tok.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn skip_braces(&mut self) -> PResult<()> {
        self.stream.expect(TokenKind::LBrace)?;
        let mut depth = 1;

        while depth > 0 {
            let tok = self.stream.next();
            match tok.kind {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => depth -= 1,
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn skip_block(&mut self) -> PResult<()> {
        self.skip_braces()
    }

    pub(crate) fn skip_generic_params(&mut self) -> PResult<()> {
        if self.stream.peek_kind() != TokenKind::LAngle {
            return Ok(());
        }

        self.stream.next(); // '<'
        let mut depth = 1;

        while depth > 0 {
            let tok = self.stream.next();
            match tok.kind {
                TokenKind::LAngle => depth += 1,
                TokenKind::RAngle => depth -= 1,
                _ => {}
            }
        }

        Ok(())
    }
}
