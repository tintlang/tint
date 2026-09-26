use crate::error::PResult;
use crate::Parser;
use tint_lexer::TokenKind;

impl Parser {
    /// Skips a function declaration during the symbol-collection pass.
    pub(crate) fn skip_fn_decl(&mut self) -> PResult<()> {
        while self.stream.peek_kind() == TokenKind::LBracket {
            self.skip_attributes()?;
        }

        if self.stream.peek_kind() == TokenKind::Ident {
            self.stream.next();
        }

        self.skip_generic_params()?;

        self.skip_parens()?;

        if self.stream.consume_if(TokenKind::Arrow) {
            self.skip_type()?;
        }

        if self.stream.consume_if(TokenKind::Eq) {
            self.skip_expr()?;
            return Ok(());
        }

        self.skip_block()
    }

    pub(crate) fn skip_attributes(&mut self) -> PResult<()> {
        self.stream.expect(TokenKind::LBracket)?; // '['
        self.stream.expect(TokenKind::At)?; // '@'
        self.stream.expect(TokenKind::LParen)?; // '('

        loop {
            self.stream.expect(TokenKind::Ident)?;
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RParen)?; // ')'
        self.stream.expect(TokenKind::RBracket)?; // ']'
        Ok(())
    }

    pub(crate) fn skip_type(&mut self) -> PResult<()> {
        match self.stream.peek_kind() {
            TokenKind::Ident => {
                let name = self.stream.next().lexeme.clone();

                if name == "union" && self.stream.consume_if(TokenKind::LParen) {
                    loop {
                        self.skip_type()?;
                        if self.stream.consume_if(TokenKind::RParen) {
                            break;
                        }
                        self.stream.expect(TokenKind::Pipe)?;
                    }
                    return Ok(());
                }
            }

            TokenKind::LParen => {
                self.stream.next();
                self.stream.expect(TokenKind::RParen)?;
                return Ok(());
            }

            _ => {
                // unexpected, but don't crash in skip mode
                self.stream.next();
                return Ok(());
            }
        }

        // generics: <T, X, union(...)>
        if self.stream.consume_if(TokenKind::LAngle) {
            let mut depth = 1;

            while depth > 0 {
                let tok = self.stream.next();
                match tok.kind {
                    TokenKind::LAngle => depth += 1,
                    TokenKind::RAngle => depth -= 1,
                    _ => {}
                }
            }
        }

        Ok(())
    }

    //  SKIP EXPRESSION (until ';' or '{')
    pub(crate) fn skip_expr(&mut self) -> PResult<()> {
        loop {
            match self.stream.peek_kind() {
                TokenKind::Semicolon | TokenKind::RBrace => {
                    return Ok(());
                }

                TokenKind::LBrace => {
                    // block expression
                    self.skip_block()?;
                    return Ok(());
                }

                TokenKind::LParen => {
                    self.skip_parens()?;
                }

                TokenKind::LBracket => {
                    self.skip_bracket_expr()?;
                }

                TokenKind::LAngle => {
                    // maybe generic in expr, skip generics
                    self.skip_generic_params()?;
                }

                TokenKind::Eof => return Ok(()),

                _ => {
                    // consume normal token
                    self.stream.next();
                }
            }
        }
    }

    // helper: skip [ ... ]
    pub(crate) fn skip_bracket_expr(&mut self) -> PResult<()> {
        self.stream.expect(TokenKind::LBracket)?;
        let mut depth = 1;

        while depth > 0 {
            let tok = self.stream.next();
            match tok.kind {
                TokenKind::LBracket => depth += 1,
                TokenKind::RBracket => depth -= 1,
                _ => {}
            }
        }
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
