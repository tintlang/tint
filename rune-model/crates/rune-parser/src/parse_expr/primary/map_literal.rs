use crate::{Parser, error::*};
use rune_ast::{Expr, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_map_literal(&mut self) -> PResult<Expr> {
        let start = self.stream.next().span; // consume `map`

        let entries = self.parse_map_entries()?;
        let end = self.stream.last_span();

        Ok(Expr::MapInit {
            entries,
            span: Span::merge(start, end),
        })
    }

    fn parse_map_entries(&mut self) -> PResult<Vec<(String, Expr)>> {
        self.stream.expect(TokenKind::LBrace)?;

        let mut entries = Vec::new();

        #[derive(PartialEq)]
        enum Style { Rune, Colon }
        let mut style: Option<Style> = None;

        // empty literal
        if self.stream.consume_if(TokenKind::RBrace) {
            return Ok(entries);
        }

        loop {
            // TRAILING COMMA HANDLING
            if self.stream.peek_kind() == TokenKind::RBrace {
                break;
            }

            let key = self.parse_ident()?;
            let key_span = self.stream.last_span();

            let next = self.stream.peek().kind.clone();

            match next {
                TokenKind::LBrace => {
                    // Rune-style: key { expr }
                    self.stream.next(); // {
                    let expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBrace)?; // }

                    match style {
                        None => style = Some(Style::Rune),
                        Some(Style::Rune) => {}
                        Some(Style::Colon) => {
                            return Err(ParserError::Message {
                                msg: "Cannot mix Colon `:` style with Rune `{}` style in map literal".into(),
                                span: key_span,
                            });
                        }
                    }

                    entries.push((key, expr));
                }

                TokenKind::Colon => {
                    // Colon-style: key : expr
                    self.stream.next(); // :
                    let expr = self.parse_expr()?;

                    match style {
                        None => style = Some(Style::Colon),
                        Some(Style::Colon) => {}
                        Some(Style::Rune) => {
                            return Err(ParserError::Message {
                                msg: "Cannot mix Rune `{}` style with Colon `:` style in map literal".into(),
                                span: key_span,
                            });
                        }
                    }

                    entries.push((key, expr));
                }

                _ => {
                    return Err(ParserError::Message {
                        msg: "Expected `{` or `:` in map entry".into(),
                        span: key_span,
                    });
                }
            }

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RBrace)?;
        Ok(entries)
    }
}
