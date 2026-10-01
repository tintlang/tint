use crate::error::*;
use crate::Parser;
use tint_ast::*;
use tint_lexer::TokenKind;

impl Parser {
    // fn name(params...) -> Type { block }
    // async fn name(params...) { ... }
    // fn name(params...) = expr
    pub(crate) fn parse_fn_decl(&mut self) -> PResult<FnDecl> {
        // async
        let async_span = if self.stream.consume_if(TokenKind::Async) {
            Some(self.stream.last_span())
        } else {
            None
        };

        // "fn"
        let start = self.stream.expect(TokenKind::Fn)?.span;

        // attributes
        let mut local_attrs = AttributeList::empty();

        if self.stream.peek_kind() == TokenKind::LBracket {
            // parse *only one* attribute block
            let a = self.parse_attributes()?;
            local_attrs.extend(a);

            // forbid second block
            if self.stream.peek_kind() == TokenKind::LBracket {
                return Err(ParserError::Message {
                    msg: "Only one attribute block allowed after `fn`. Use [@(a, b)] instead of multiple blocks."
                        .into(),
                    span: self.stream.peek().span,
                });
            }
        }

        // NAME
        let name = self.parse_ident()?;

        // GENERICS: <T, U>
        let generics = self.parse_optional_generics()?;

        // Parameters are patterns, not only identifiers.
        let params = self.parse_fn_params()?;

        // -------- RETURN TYPE --------
        let ret_ty = if self.stream.consume_if(TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        // -------- SHORT FORM: fn x() = expr --------
        if self.stream.consume_if(TokenKind::Eq) {
            let expr = self.parse_expr()?;
            let span = Span::merge(start, expr.span());

            return Ok(FnDecl {
                attributes: local_attrs,
                name,
                generics,
                params,
                ret_ty,
                async_: async_span.is_some(),
                body: FnBody::Expr(expr),
                exported: false,
                span,
            });
        }

        // -------- { BLOCK } --------
        let block = if async_span.is_some() { self.parse_async_block()? } else { self.parse_block()? };
        let span = Span::merge(start, block.span);

        Ok(FnDecl {
            attributes: local_attrs,
            name,
            generics,
            params,
            ret_ty,
            async_: async_span.is_some(),
            body: FnBody::Block(block),
            exported: false,
            span,
        })
    }

    // PARAMS: (pattern: Type {default}, ...)
    //
    // supports:
    // fn f((x,y)) {}
    // fn f(User { id, name }) {}
    // fn f(_, x, (a,(b,c))) {}
    // fn f(x: i32) {}
    // fn f(x {10}) {}
    fn parse_fn_params(&mut self) -> PResult<Vec<Param>> {
        self.stream.expect(TokenKind::LParen)?;

        if self.stream.consume_if(TokenKind::RParen) {
            return Ok(vec![]);
        }

        #[derive(PartialEq)]
        enum Style {
            Tint,
            Typed,
        }
        let mut style: Option<Style> = None;

        let mut params = Vec::new();

        loop {
            // 1) PATTERN
            self.in_pattern = true;
            let parsed_pat = if self.stream.consume_if(TokenKind::Ampersand) {
                let mutable = self.stream.peek().lexeme == "mut";
                if mutable {
                    self.stream.next();
                }
                let self_token = self.stream.expect(TokenKind::SelfKw)?;
                let self_pattern = Pattern::Ident("self".into(), self_token.span);
                if mutable {
                    Pattern::Mut {
                        span: Span::merge(self_token.span, self_token.span),
                        inner: Box::new(self_pattern),
                    }
                } else {
                    self_pattern
                }
            } else {
                self.parse_pattern()?
            };
            self.in_pattern = false;

            // `parse_pattern` owns the `name: Type` form and therefore may
            // already have consumed the type before control gets here.  Pull
            // that type back out so defaults (`x: T = expr`) are parsed by
            // this parameter parser instead of being mistaken for trailing
            // tokens after the pattern.
            let inferred_ty = match &parsed_pat {
                Pattern::Typed { ty, .. } => Some(ty.clone()),
                _ => None,
            };
            let pat = parsed_pat;

            let typed_pattern = inferred_ty.is_some();
            if typed_pattern {
                match style {
                    None => style = Some(Style::Typed),
                    Some(Style::Typed) => {}
                    Some(Style::Tint) => {
                        return Err(ParserError::Message {
                            msg: "Cannot mix typed parameters (`x: T`) with Tint parameters (`x {expr}`)".into(),
                            span: self.stream.last_span(),
                        });
                    }
                }
            }

            let mut ty = inferred_ty;
            let mut default: Option<DefaultValue> = None;

            // 2) optional type: x: T. Simple identifier patterns have
            // already consumed this in parse_pattern; this branch remains
            // for patterns whose type annotation is parsed here.
            if !typed_pattern && self.stream.consume_if(TokenKind::Colon) {
                match style {
                    None => style = Some(Style::Typed),
                    Some(Style::Typed) => {}
                    Some(Style::Tint) => {
                        return Err(ParserError::Message {
                            msg: "Cannot mix typed parameters (`x: T`) with Tint parameters (`x {expr}`)".into(),
                            span: self.stream.last_span(),
                        });
                    }
                }
                ty = Some(self.parse_type()?);
            }

            // Canonical typed default: x: T = expr. This also handles the
            // simple identifier case where parse_pattern consumed `: T`.
            if self.stream.consume_if(TokenKind::Eq) {
                let expr = self.parse_expr()?;
                default = Some(DefaultValue::Single(expr));
            }
            // Legacy Tint-style default: pattern {expr}. A typed pattern may
            // keep this old spelling as a compatibility form, but an
            // untyped parameter still selects the Tint style and cannot be
            // mixed with typed parameters in the same signature.
            else if self.stream.consume_if(TokenKind::LBrace) {
                if typed_pattern {
                    let expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBrace)?;
                    default = Some(DefaultValue::Single(expr));
                } else {
                    match style {
                        None => style = Some(Style::Tint),
                        Some(Style::Tint) => {}
                        Some(Style::Typed) => {
                            return Err(ParserError::Message {
                            msg: "Cannot mix Tint-style defaults (`x {expr}`) with typed parameters (`x: T = expr`)".into(),
                            span: self.stream.last_span(),
                        });
                        }
                    }

                    let expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBrace)?;

                    // SINGLE vs BROADCAST
                    let simple_ident = match &pat {
                        Pattern::Ident(_, _) => true,
                        Pattern::Typed { pat, .. } => {
                            matches!(pat.as_ref(), Pattern::Ident(_, _))
                        }
                        _ => false,
                    };
                    if simple_ident {
                        default = Some(DefaultValue::Single(expr));
                    } else {
                        default = Some(DefaultValue::Broadcast(expr));
                    }
                }
            }

            // push parameter
            params.push(Param {
                pattern: pat,
                ty,
                default,
            });

            // , or end
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        self.stream.expect(TokenKind::RParen)?;
        Ok(params)
    }
}

impl Parser {
    /// Body of an `async fn`. `await` is sugar for a trailing callback:
    ///
    /// ```text
    /// let r = await fetch(url);   ==>   fetch(url, |r| { <rest of the block> });
    /// await save(r);              ==>   save(r, |_| { <rest of the block> });
    /// ```
    ///
    /// Only the top-level statements of the fn body may use it.
    fn parse_async_block(&mut self) -> PResult<Block> {
        let start = self.stream.expect(TokenKind::LBrace)?.span;
        let block = self.parse_async_stmts(start)?;
        Ok(block)
    }

    // Parses statements up to and including the closing `}`.
    fn parse_async_stmts(&mut self, start: Span) -> PResult<Block> {
        let mut stmts = Vec::new();
        let mut last_span = start;
        while !self.stream.consume_if(TokenKind::RBrace) {
            // `await call(..)` or `let name = await call(..)`
            let binding = if self.stream.peek_kind() == TokenKind::Await {
                Some("_".to_string())
            } else if self.stream.peek_kind() == TokenKind::Let
                && self.stream.peek_n_kind(1) == TokenKind::Ident
                && self.stream.peek_n_kind(2) == TokenKind::Eq
                && self.stream.peek_n_kind(3) == TokenKind::Await
            {
                let name = self.stream.peek_n(1).lexeme.clone();
                for _ in 0..3 {
                    self.stream.next();
                }
                Some(name)
            } else {
                None
            };
            let Some(binding) = binding else {
                let stmt = self.parse_stmt()?;
                last_span = stmt.span();
                stmts.push(stmt);
                self.stream.consume_if(TokenKind::Semicolon);
                continue;
            };
            let await_span = self.stream.expect(TokenKind::Await)?.span;
            let call = self.parse_expr()?;
            self.stream.consume_if(TokenKind::Semicolon);
            let Expr::Call { target, mut args, span } = call else {
                return Err(ParserError::Message {
                    msg: "`await` needs a function call: `await name(args)`".into(),
                    span: await_span,
                });
            };
            let rest = self.parse_async_stmts(start)?;
            let rest_span = rest.span;
            args.push(Expr::Lambda {
                params: vec![binding],
                body: Box::new(Expr::Block(rest, rest_span)),
                span: rest_span,
            });
            let call = Expr::Call { target, args, span: Span::merge(span, rest_span) };
            last_span = call.span();
            stmts.push(Stmt::Expr(call));
            return Ok(Block { stmts, span: Span::merge(start, last_span) });
        }
        Ok(Block { stmts, span: Span::merge(start, last_span) })
    }
}
