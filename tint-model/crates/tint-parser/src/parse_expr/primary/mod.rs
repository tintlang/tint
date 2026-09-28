mod array_lit;
mod bool_lit;
mod ident_or_struct;
mod lambda;
mod map_literal;
mod number;
mod paren_or_tuple;
mod string;
mod unary;

use crate::{error::*, Parser};
use tint_ast::Expr;
use tint_ast::Span;
use tint_ast::StructInitField;
use tint_lexer::TokenKind;
use tint_lexer::TokenKind::Colon;
use tint_lexer::TokenKind::Comma;
use tint_lexer::TokenKind::LBrace;
use tint_lexer::TokenKind::RBrace;

impl Parser {
    /// Parse `{ a{1}, b{2}, c: 3 }`
    pub(crate) fn parse_struct_init_fields(&mut self) -> PResult<Vec<StructInitField>> {
        self.stream.expect(LBrace)?;
        let mut fields = Vec::new();

        if self.stream.consume_if(RBrace) {
            return Ok(fields);
        }

        loop {
            // name
            let name = self.parse_ident()?;
            let start = self.stream.last_span();

            match self.stream.peek_kind() {
                // Tint style:  x{expr}
                LBrace => {
                    self.stream.next(); // '{'
                    let expr = self.parse_expr()?;
                    self.stream.expect(RBrace)?;
                    fields.push(StructInitField::Tint {
                        name,
                        expr,
                        span: Span::merge(start, self.stream.last_span()),
                    });
                }

                // Rust style:  x: expr
                Colon => {
                    self.stream.next(); // ':'
                    let expr = self.parse_expr()?;
                    fields.push(StructInitField::Assign {
                        name,
                        expr,
                        span: Span::merge(start, self.stream.last_span()),
                    });
                }

                other => {
                    return Err(ParserError::Message {
                        msg: format!("Expected `{{` or `:` in struct field, found {:?}", other),
                        span: self.stream.peek().span,
                    });
                }
            }

            if !self.stream.consume_if(Comma) {
                break;
            }
        }

        self.stream.expect(RBrace)?;
        Ok(fields)
    }

    pub(crate) fn parse_primary(&mut self) -> PResult<Expr> {
        let tok = self.stream.peek().clone();

        if self.stream.peek_kind() == TokenKind::Borrow {
            return self.parse_borrow_expr();
        }

        match tok.kind {
            TokenKind::If => self.parse_if_expr(),
            TokenKind::Number => self.parse_number(),
            TokenKind::String => self.parse_string_or_interpolated(),
            TokenKind::True | TokenKind::False => self.parse_bool(),
            // ENUM VARIANT INIT:  EnumName::Variant { ... } ---
            TokenKind::Ident => {
                // peek next: Ident '::'
                let name = self.stream.peek().lexeme.clone();

                if self.stream.peek2_kind() == TokenKind::PathSep {
                    let enum_name = name;

                    self.stream.next(); // ident
                    self.stream.next(); // ::

                    let variant = self.parse_ident()?;

                    // ENUM VARIANT INIT
                    if self.stream.peek_kind() == TokenKind::LBrace {
                        let start = self.stream.last_span();
                        let fields = self.parse_struct_init_fields()?;
                        let end = self.stream.last_span();

                        return Ok(Expr::VariantInit {
                            enum_name,
                            variant,
                            fields,
                            span: Span::merge(start, end),
                        });
                    }

                    // Enum constructors also support the two compact forms
                    // useful for state machines and events:
                    //
                    //   GameState::Paused
                    //   GameEvent::Move(10, 20)
                    //
                    // Uppercase variant names distinguish these from module
                    // namespace expressions such as `math::abs`.
                    let is_variant = variant
                        .chars()
                        .next()
                        .map(|c| c.is_uppercase())
                        .unwrap_or(false);

                    if is_variant && self.stream.peek_kind() == TokenKind::LParen {
                        let start = self.stream.next().span;
                        let mut fields = Vec::new();

                        if !self.stream.consume_if(TokenKind::RParen) {
                            loop {
                                let expr = self.parse_expr()?;
                                let index = fields.len();
                                fields.push(StructInitField::Assign {
                                    name: format!("_{index}"),
                                    expr,
                                    span: start,
                                });

                                if !self.stream.consume_if(Comma) {
                                    break;
                                }
                            }
                            self.stream.expect(TokenKind::RParen)?;
                        }

                        let end = self.stream.last_span();
                        return Ok(Expr::VariantInit {
                            enum_name,
                            variant,
                            fields,
                            span: Span::merge(start, end),
                        });
                    }

                    if is_variant {
                        let span = Span::merge(tok.span, self.stream.last_span());
                        return Ok(Expr::VariantInit {
                            enum_name,
                            variant,
                            fields: Vec::new(),
                            span,
                        });
                    }

                    // Fall back to a namespace expression and allow postfix parsing.
                    let expr = Expr::Namespace {
                        base: Box::new(Expr::Ident(enum_name.clone(), tok.span)),
                        item: variant.clone(),
                        span: Span::merge(tok.span, self.stream.last_span()),
                    };

                    return self.parse_postfix_with(expr);
                }

                // normal identifier
                self.parse_ident_or_struct()
            }

            TokenKind::Match => self.parse_match_expression(),
            TokenKind::Pipe => self.parse_lambda(),

            TokenKind::LBracket => self.parse_array_literal(),
            TokenKind::LParen => self.parse_paren_or_tuple(),

            TokenKind::MapLit => self.parse_map_literal(),

            TokenKind::Bang | TokenKind::Minus => self.parse_unary(),

            TokenKind::LBrace => {
                let block = self.parse_block()?;
                let span = block.span; // block already contains its full span
                return Ok(Expr::Block(block, span));
            }

            TokenKind::SelfKw => {
                let tok = self.stream.next();
                return Ok(Expr::SelfKw(tok.span));
            }

            TokenKind::Kernel => {
                let start = self.stream.next().span;

                let name = self.parse_ident()?;
                let end = self.stream.last_span();

                let span = Span::merge(start, end);

                Ok(Expr::Ident(name, span))
            }

            _ => Err(ParserError::Message {
                msg: format!("Unexpected token in expression: {:?}", tok.kind),
                span: tok.span,
            }),
        }
    }

    fn parse_if_expr(&mut self) -> PResult<Expr> {
        let start = self.stream.next().span;
        let cond = self.parse_expr()?;
        let then = self.parse_block()?;
        self.stream.expect(TokenKind::Else)?;
        let else_ = self.parse_block()?;
        let span = Span::merge(start, else_.span);
        Ok(Expr::If {
            cond: Box::new(cond),
            then,
            else_,
            span,
        })
    }
}
