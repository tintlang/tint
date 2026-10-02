use super::*;

impl Parser {
    pub fn parse_block_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.peek().span;
        let name = self.parse_ident()?;

        self.stream.expect(TokenKind::LBrace)?;

        let (attributes, modifiers) = self.parse_modifier_list_block()?;
        self.ui_block_columns.push(start.start.column);
        let children = self.parse_block_children();
        self.ui_block_columns.pop();
        let children = children?;

        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiNode::BlockElement {
            name,
            attributes,
            modifiers,
            children,
            span: Span::merge(start, end),
        })
    }

    pub(crate) fn parse_block_children(&mut self) -> PResult<Vec<UiNodeOrExpr>> {
        let mut out = Vec::new();

        loop {
            if self.recover_ui_indentation
                && self.stream.peek().kind == TokenKind::Ident
                && self.stream.peek2_kind() == TokenKind::LBrace
                && self
                    .ui_block_columns
                    .last()
                    .is_some_and(|column| self.stream.peek().span.start.column <= *column)
            {
                return Err(ParserError::Message {
                    msg: format!(
                        "Expected `}}` before UI node `{}`. This node starts at the same level as the current block.",
                        self.stream.peek().lexeme
                    ),
                    span: self.stream.peek().span,
                });
            }
            match self.stream.peek().kind {
                TokenKind::RBrace => break,
                TokenKind::Ident
                    if self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident
                        && self.stream.peek_n_kind(3) == TokenKind::LBrace
                        && self.stream.peek().lexeme == "theme" =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_theme_node()?));
                }
                // `case <label> { ... }` -- a match{}-arm child, block
                // mode's counterpart to the old XML dialect's `<case
                // label>...</case>` (see `parse_block_case_node` below).
                // Checked before the generic child-tag branch just under
                // this one: a bare `case { ... }` (no label) has no
                // `Ident`/`Underscore` in between, so it falls through to
                // that generic branch and parses as an ordinary tag named
                // "case", same as any other component name would.
                TokenKind::Ident
                    if self.stream.peek().lexeme == "case"
                        && matches!(
                            self.stream.peek2_kind(),
                            TokenKind::Ident | TokenKind::Underscore
                        )
                        && self.stream.peek_n_kind(2) == TokenKind::LBrace =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_block_case_node()?));
                }

                TokenKind::Ident
                    if self.stream.peek().lexeme == "style"
                        && self.stream.peek2_kind() == TokenKind::Ident
                        && self.stream.peek_n_kind(2) == TokenKind::LBrace =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_style_node()?));
                }

                TokenKind::Ident
                    if self.stream.peek().lexeme == "component"
                        && self.stream.peek2_kind() == TokenKind::Ident
                        && matches!(self.stream.peek_n_kind(2), TokenKind::LBrace | TokenKind::LParen) =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_component_node()?));
                }

                TokenKind::Ident
                    if self.stream.peek().lexeme == "variant"
                        && self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident
                        && self.stream.peek_n_kind(3) == TokenKind::LBrace =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_variant_node()?));
                }

                TokenKind::Ident
                    if self.stream.peek().lexeme == "slot"
                        && (self.stream.peek2_kind() == TokenKind::Ident
                            || (self.stream.peek2_kind() == TokenKind::PathSep
                                && self.stream.peek_n_kind(2) == TokenKind::Ident)) =>
                {
                    out.push(UiNodeOrExpr::Node(self.parse_slot_node()?));
                }

                TokenKind::Ident if self.stream.peek2_kind() == TokenKind::LBrace => {
                    out.push(UiNodeOrExpr::Node(self.parse_block_node()?));
                }

                // Standalone `for { var in iterable } { ...body... }` --
                // a child in its own right, not the `for{}` modifier
                // parsed inside `parse_modifier_list_block` below (that one
                // only ever appears right after a node's `{`, before any
                // children; this one appears IN the children list itself,
                // so the two can't collide).
                TokenKind::For => {
                    out.push(UiNodeOrExpr::Node(self.parse_block_for_node()?));
                }

                TokenKind::String => {
                    let tok = self.stream.next().clone();
                    let parsed = self.parse_interpolated_text(tok.lexeme.clone(), tok.span)?;
                    out.push(UiNodeOrExpr::Text(parsed));
                }

                _ => break,
            }
        }

        Ok(out)
    }

    pub(crate) fn parse_modifier_list_block(
        &mut self,
    ) -> PResult<(Vec<UiAttribute>, Vec<UiModifier>)> {
        let mut attrs = Vec::new();
        let mut mods = Vec::new();

        while matches!(self.stream.peek().kind, TokenKind::Ident | TokenKind::Use) {
            if self.stream.peek().lexeme == "slot"
                && (self.stream.peek2_kind() == TokenKind::Ident
                    || (self.stream.peek2_kind() == TokenKind::PathSep
                        && self.stream.peek_n_kind(2) == TokenKind::Ident))
            {
                break;
            }
            if self.stream.peek().lexeme == "variant"
                && self.stream.peek2_kind() == TokenKind::PathSep
                && self.stream.peek_n_kind(2) == TokenKind::Ident
                && self.stream.peek_n_kind(3) == TokenKind::LBrace
            {
                break;
            }
            // `theme::dark { ... }` is a themed child block, not the
            // `theme::dark` modifier followed by an unexpected brace.
            if self.stream.peek().lexeme == "theme"
                && self.stream.peek2_kind() == TokenKind::PathSep
                && self.stream.peek_n_kind(2) == TokenKind::Ident
                && self.stream.peek_n_kind(3) == TokenKind::LBrace
            {
                break;
            }

            // `Ident {` starts a child node, so stop modifier parsing before it.
            if self.stream.peek2_kind() == TokenKind::LBrace {
                break;
            }

            let tok = self.stream.next();
            let name = tok.lexeme.clone();
            let span = tok.span;

            if self.stream.peek().kind == TokenKind::OrOr {
                self.stream.next();
                let value = self.parse_attribute_rhs()?;

                attrs.push(UiAttribute { name, value, span });

                continue;
            }

            let mut path = vec![name];

            while self.stream.peek().kind == TokenKind::Dot {
                self.stream.next(); // consume '.'
                let seg = self.parse_ident()?;
                path.push(seg);
            }

            if self.stream.peek().kind == TokenKind::PathSep {
                self.stream.next();

                let value = self.parse_modifier_value_rhs()?;

                mods.push(UiModifier { path, value, span });

                continue;
            }

            break;
        }
        // Control-flow modifiers are parsed after regular attributes and modifiers.
        loop {
            match self.stream.peek().kind {
                TokenKind::If => {
                    let tok = self.stream.next();
                    let span = tok.span;

                    self.stream.expect(TokenKind::LBrace)?;
                    let expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBrace)?;

                    mods.push(UiModifier {
                        path: vec!["if".to_string()],
                        value: UiModifierValue::Expr(expr),
                        span,
                    });
                }
                TokenKind::For => {
                    let tok = self.stream.next();
                    let span = tok.span;

                    self.stream.expect(TokenKind::LBrace)?;

                    // The loop variable is bound per-iteration by the builder
                    // (see tint-runtime/src/ui/builder.rs) -- it's no longer
                    // just parsed-then-discarded, it travels with the iterable
                    // as a MiniMod{key: [var], value: Expr(iterable)}.
                    let var = self.parse_ident()?;

                    self.stream.expect(TokenKind::In)?;

                    let expr = self.parse_expr()?;

                    self.stream.expect(TokenKind::RBrace)?;

                    mods.push(UiModifier {
                        path: vec!["for".to_string()],
                        value: UiModifierValue::MiniMod {
                            key: vec![var],
                            value: Box::new(UiModifierValue::Expr(expr)),
                        },
                        span,
                    });
                }
                TokenKind::Match => {
                    let tok = self.stream.next();
                    let span = tok.span;

                    self.stream.expect(TokenKind::LBrace)?;
                    let expr = self.parse_expr()?;
                    self.stream.expect(TokenKind::RBrace)?;

                    mods.push(UiModifier {
                        path: vec!["match".to_string()],
                        value: UiModifierValue::Expr(expr),
                        span,
                    });
                }
                _ => break,
            }
        }

        Ok((attrs, mods))
    }

    pub(crate) fn parse_style_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.next().span;
        let name = self.parse_ident()?;
        self.stream.expect(TokenKind::LBrace)?;
        let (_, modifiers) = self.parse_modifier_list_block()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiNode::Style {
            name,
            modifiers,
            span: Span::merge(start, end),
        })
    }

    pub(crate) fn parse_component_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.next().span;
        let name = self.parse_ident()?;
        // `component Name(label: string, start: i32 {0}) { ... }`
        let params = if self.stream.consume_if(TokenKind::LParen) {
            let params = self.parse_params()?;
            self.stream.expect(TokenKind::RParen)?;
            params
        } else {
            Vec::new()
        };
        self.stream.expect(TokenKind::LBrace)?;
        // `state x = expr` and `fn name() { .. }` lines come first, as in a `ui fn`.
        let (mut state, mut fns) = (Vec::new(), Vec::new());
        let mut resource_attrs: Vec<UiAttribute> = Vec::new();
        loop {
            match self.stream.peek().kind {
                TokenKind::State => {
                    let decl_start = self.stream.next().span;
                    let name = self.parse_ident()?;
                    self.stream.expect(TokenKind::Eq)?;
                    let init = self.parse_expr()?;
                    state.push(UiStateDecl { name, init, derived: false, persist: false, span: Span::merge(decl_start, self.stream.last_span()) });
                }
                TokenKind::Ident if self.at_form() => self.parse_form(&mut state, &mut fns)?,
                TokenKind::Ident if self.at_persist() => {
                    self.stream.next();
                    let decl_start = self.stream.next().span;
                    let name = self.parse_ident()?;
                    self.stream.expect(TokenKind::Eq)?;
                    let init = self.parse_expr()?;
                    state.push(UiStateDecl { name, init, derived: false, persist: true, span: Span::merge(decl_start, self.stream.last_span()) });
                }
                TokenKind::Ident if self.at_derived() => {
                    let decl_start = self.stream.next().span;
                    let name = self.parse_ident()?;
                    self.stream.expect(TokenKind::Eq)?;
                    let init = self.parse_expr()?;
                    state.push(UiStateDecl { name, init, derived: true, persist: false, span: Span::merge(decl_start, self.stream.last_span()) });
                }
                TokenKind::Fn | TokenKind::Async => fns.push(self.parse_fn_decl()?),
                TokenKind::Ident if self.stream.peek().lexeme == "resource" => {
                    self.parse_resource(&mut state, &mut fns, &mut resource_attrs)?
                }
                _ => break,
            }
        }
        let (mut attributes, modifiers) = self.parse_modifier_list_block()?;
        if let Some(a) = attributes
            .iter()
            .find(|a| !tint_ast::ui_attrs::LIFECYCLE_ATTRS.contains(&a.name.as_str()) && !tint_ast::ui_attrs::VALUE_ATTRS.contains(&a.name.as_str()))
        {
            return Err(crate::error::ParserError::Message {
                msg: "A component declaration can only have `mount||`, `unmount||`, `effect||`, `deps||`, `role||` and `aria_*||` attributes".into(),
                span: a.span,
            });
        }
        attributes.splice(0..0, resource_attrs);
        let children = self.parse_block_children()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiNode::Component {
            name,
            params,
            state,
            fns,
            attributes,
            modifiers,
            children,
            span: Span::merge(start, end),
        })
    }

    /// A `form name handler { field .. }` block is next.
    pub(crate) fn at_form(&self) -> bool {
        self.stream.peek().kind == TokenKind::Ident
            && self.stream.peek().lexeme == "form"
            && self.stream.peek_n_kind(1) == TokenKind::Ident
            && self.stream.peek_n_kind(2) == TokenKind::Ident
            && self.stream.peek_n_kind(3) == TokenKind::LBrace
    }

    /// `form login save { field email = "" "required, email"  field pw = "" "required, min:8" }`
    /// becomes plain state, derived values and handlers (see docs/guide/forms.md):
    /// `login_email`, `login_email_touched`, `login_email_error`, `login_email_msg`,
    /// `login_email_set(v)`, `login_email_blur()`, `login_submitted`, `login_submitting`,
    /// `login_valid`, `login_submit()`, `login_done()`, `login_reset()`.
    fn parse_form(&mut self, state: &mut Vec<UiStateDecl>, fns: &mut Vec<FnDecl>) -> PResult<()> {
        let start = self.stream.next().span;
        let form = self.parse_ident()?;
        let handler = self.parse_ident()?;
        self.stream.expect(TokenKind::LBrace)?;
        let mut fields: Vec<(String, Expr, String)> = Vec::new();
        while self.stream.peek().kind == TokenKind::Ident && self.stream.peek().lexeme == "field" {
            self.stream.next();
            let name = self.parse_ident()?;
            self.stream.expect(TokenKind::Eq)?;
            let init = self.parse_expr()?;
            let rules = if self.stream.peek().kind == TokenKind::String {
                let t = self.stream.next();
                t.lexeme.trim_matches('"').to_string()
            } else {
                String::new()
            };
            fields.push((name, init, rules));
        }
        self.stream.expect(TokenKind::RBrace)?;
        let span = Span::merge(start, self.stream.last_span());
        if fields.is_empty() {
            return Err(crate::error::ParserError::Message { msg: "a form needs at least one `field`".into(), span });
        }
        let expr_of = |src: &str| Parser::new_expr_only(src.to_string(), span).parse_expr();
        let fn_of = |src: String| Parser::new_expr_only(src, span).parse_fn_decl();
        let mut push = |name: String, init: Expr, derived: bool| state.push(UiStateDecl { name, init, derived, persist: false, span });
        let mut valid: Vec<String> = Vec::new();
        for (x, init, rules) in &fields {
            let v = format!("{form}_{x}");
            push(v.clone(), init.clone(), false);
            push(format!("{v}_touched"), expr_of("false")?, false);
            let mut chain = "\"\"".to_string();
            for rule in rules.split(',').map(str::trim).filter(|r| !r.is_empty()).collect::<Vec<_>>().into_iter().rev() {
                let (name, arg) = rule.split_once(':').map(|(a, b)| (a.trim(), b.trim())).unwrap_or((rule, ""));
                let (cond, msg) = match name {
                    "required" => (format!("{v}.trim().is_empty()"), "Required".to_string()),
                    "email" => (format!("!{v}.is_empty() && !({v}.contains(\"@\") && {v}.contains(\".\"))"), "Enter a valid email".to_string()),
                    "min" => (format!("!{v}.is_empty() && {v}.len() < {arg}"), format!("At least {arg} characters")),
                    "max" => (format!("{v}.len() > {arg}"), format!("At most {arg} characters")),
                    "same" => (format!("{v} != {form}_{arg}"), "Does not match".to_string()),
                    other => return Err(crate::error::ParserError::Message { msg: format!("unknown form rule `{other}` (required, email, min:N, max:N, same:field)"), span }),
                };
                chain = format!("if {cond} {{ \"{msg}\" }} else {{ {chain} }}");
            }
            push(format!("{v}_error"), expr_of(&chain)?, true);
            push(format!("{v}_msg"), expr_of(&format!("if {v}_touched || {form}_submitted {{ {v}_error }} else {{ \"\" }}"))?, true);
            fns.push(fn_of(format!("fn {v}_set(v: string) {{ {v} = v }}"))?);
            fns.push(fn_of(format!("fn {v}_blur() {{ {v}_touched = true }}"))?);
            valid.push(format!("{v}_error == \"\""));
        }
        push(format!("{form}_submitted"), expr_of("false")?, false);
        push(format!("{form}_submitting"), expr_of("false")?, false);
        push(format!("{form}_valid"), expr_of(&valid.join(" && "))?, true);
        fns.push(fn_of(format!("fn {form}_submit() {{ {form}_submitted = true\n if {form}_valid {{ {form}_submitting = true\n {handler}() }} }}"))?);
        fns.push(fn_of(format!("fn {form}_done() {{ {form}_submitting = false }}"))?);
        let mut reset = fn_of(format!("fn {form}_reset() {{ {form}_submitted = false\n {form}_submitting = false }}"))?;
        if let FnBody::Block(b) = &mut reset.body {
            for (x, init, _) in &fields {
                b.stmts.push(Stmt::Assign { lhs: Expr::Ident(format!("{form}_{x}"), span), rhs: init.clone(), span });
                b.stmts.push(Stmt::Assign { lhs: Expr::Ident(format!("{form}_{x}_touched"), span), rhs: expr_of("false")?, span });
            }
        }
        fns.push(reset);
        Ok(())
    }

    /// A `persist state name = expr` line is next.
    pub(crate) fn at_persist(&self) -> bool {
        self.stream.peek().kind == TokenKind::Ident
            && self.stream.peek().lexeme == "persist"
            && self.stream.peek_n_kind(1) == TokenKind::State
    }

    /// A `derived name = expr` line is next (`derived` is an ordinary name elsewhere).
    pub(crate) fn at_derived(&self) -> bool {
        self.stream.peek().kind == TokenKind::Ident
            && self.stream.peek().lexeme == "derived"
            && self.stream.peek_n_kind(1) == TokenKind::Ident
            && self.stream.peek_n_kind(2) == TokenKind::Eq
    }

    /// `resource name = host_fn(args)` (or `resource name: number = ..`): state `name`, `name_loading`
    /// and `name_error`, plus `fn name_load()` that calls the host function with a callback. The
    /// component runs it when it mounts; call `name_load()` again to refetch.
    fn parse_resource(
        &mut self,
        state: &mut Vec<UiStateDecl>,
        fns: &mut Vec<FnDecl>,
        attrs: &mut Vec<UiAttribute>,
    ) -> PResult<()> {
        let start = self.stream.next().span;
        let n = self.parse_ident()?;
        let ty = if self.stream.consume_if(TokenKind::Colon) { self.parse_ident()? } else { "string".to_string() };
        let (empty, ty) = match ty.as_str() {
            "string" => ("\"\"", "string"),
            "number" => ("0", "number"),
            "bool" => ("false", "bool"),
            _ => return self.stream.error_here("a resource has the type `string`, `number` or `bool`"),
        };
        self.stream.expect(TokenKind::Eq)?;
        let call = self.parse_expr()?;
        // `cache::ms` keeps the value for the page (fresh for `ms`, then shown while it is fetched
        // again), `every::ms` fetches it again on that interval while the component is on the page.
        let (mut cache, mut every): (Option<String>, Option<String>) = (None, None);
        while self.stream.peek().kind == TokenKind::Ident
            && matches!(self.stream.peek().lexeme.as_str(), "cache" | "every")
            && self.stream.peek_n_kind(1) == TokenKind::PathSep
        {
            let word = self.stream.next().lexeme.clone();
            self.stream.next();
            let num = self.stream.next();
            if num.kind != TokenKind::Number {
                return self.stream.error_here("`cache::` and `every::` take a number of milliseconds");
            }
            if word == "cache" { cache = Some(num.lexeme.clone()) } else { every = Some(num.lexeme.clone()) }
        }
        let span = Span::merge(start, self.stream.last_span());
        let Expr::Call { target, args, .. } = call else {
            return Err(crate::error::ParserError::Message { msg: "a resource is a call: `resource name = fetch_text(url)`".into(), span });
        };
        let expr_of = |src: &str| Parser::new_expr_only(src.to_string(), span).parse_expr();
        for (suffix, init) in [("", empty), ("_loading", "true"), ("_error", "\"\"")] {
            state.push(UiStateDecl { name: format!("{n}{suffix}"), init: expr_of(init)?, derived: false, persist: false, span });
        }
        let request = format!(
            "__resource(|r: Result<{ty}, string>| {{ match r {{ Ok {{ value }} => {{ {n} = value\n {}{n}_loading = false }}, Err {{ error }} => {{ {n}_error = error\n {n}_loading = false }} }} }})",
            if cache.is_some() { format!("cache_set({n}_key, value)\n ") } else { String::new() }
        );
        let mut extra: Option<String> = None;
        let src = match &cache {
            None => format!("fn {n}_load() {{ {n}_loading = true\n {n}_error = \"\"\n {request} }}"),
            Some(fresh) => {
                if ty != "string" {
                    return Err(crate::error::ParserError::Message { msg: "`cache` works for a `string` resource".into(), span });
                }
                let first = args.first().cloned().unwrap_or_else(|| Expr::String(String::new(), span));
                let key = Expr::Binary { left: Box::new(Expr::String(format!("{n}:"), span)), op: "+".into(), right: Box::new(first), span };
                state.push(UiStateDecl { name: format!("{n}_key"), init: key, derived: true, persist: false, span });
                // `n_fetch` always asks again (the poll uses it); `n_load` shows the cached value
                // and asks again only when it is older than `fresh`.
                extra = Some(format!("fn {n}_fetch() {{ {n}_error = \"\"\n {request} }}"));
                format!(
                    "fn {n}_load() {{ {n}_error = \"\"\n if cache_fresh({n}_key, 1000000000000) {{ {n} = cache_get({n}_key)\n {n}_loading = false }} else {{ {n}_loading = true }}\n if !cache_fresh({n}_key, {fresh}) {{ {request} }} }}"
                )
            }
        };
        let mut decl = Parser::new_expr_only(src, span).parse_fn_decl()?;
        let mut fetch = match extra {
            Some(src) => Some(Parser::new_expr_only(src, span).parse_fn_decl()?),
            None => None,
        };
        fn fill(stmts: &mut [Stmt], target: &Expr, args: &[Expr]) {
            for stmt in stmts {
                match stmt {
                    Stmt::Expr(Expr::Call { target: t, args: a, .. }) if matches!(&**t, Expr::Ident(id, _) if id == "__resource") => {
                        let callback = a.remove(0);
                        *t = Box::new(target.clone());
                        *a = args.to_vec();
                        a.push(callback);
                    }
                    Stmt::If { then, else_, .. } => {
                        fill(&mut then.stmts, target, args);
                        if let Some(e) = else_ {
                            fill(&mut e.stmts, target, args);
                        }
                    }
                    _ => {}
                }
            }
        }
        if let FnBody::Block(b) = &mut decl.body {
            fill(&mut b.stmts, &target, &args);
        }
        if let Some(FnDecl { body: FnBody::Block(b), .. }) = &mut fetch {
            fill(&mut b.stmts, &target, &args);
        }
        fns.push(decl);
        let poll_fn = if let Some(f) = fetch {
            let name = f.name.clone();
            fns.push(f);
            name
        } else {
            format!("{n}_load")
        };
        attrs.push(UiAttribute { name: "mount".into(), value: UiAttrValue::Ident(format!("{n}_load")), span });
        if let Some(ms) = every {
            attrs.push(UiAttribute { name: "poll".into(), value: UiAttrValue::Ident(poll_fn), span });
            attrs.push(UiAttribute { name: "every".into(), value: UiAttrValue::Literal(ms), span });
        }
        Ok(())
    }

    pub(crate) fn parse_slot_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.next().span;
        self.stream.consume_if(TokenKind::PathSep);
        let name = self.parse_ident()?;
        let children = if self.stream.consume_if(TokenKind::LBrace) {
            let children = self.parse_block_children()?;
            self.stream.expect(TokenKind::RBrace)?;
            children
        } else {
            Vec::new()
        };

        Ok(UiNode::Slot {
            name,
            children,
            span: Span::merge(start, self.stream.last_span()),
        })
    }

    pub(crate) fn parse_variant_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.next().span;
        self.stream.expect(TokenKind::PathSep)?;
        let name = self.parse_ident()?;
        self.stream.expect(TokenKind::LBrace)?;
        let (attributes, modifiers) = self.parse_modifier_list_block()?;
        if !attributes.is_empty() {
            return self
                .stream
                .error_here("Variant declarations cannot have event attributes");
        }
        let children = self.parse_block_children()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiNode::Variant {
            name,
            modifiers,
            children,
            span: Span::merge(start, end),
        })
    }
    /// `for { var in iterable } { ...body... }` as a standalone child --
    /// see `UiNode::For`'s doc comment for how this differs from the
    /// `for{}` modifier parsed above in `parse_modifier_list_block`.
    pub(crate) fn parse_block_for_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.next().span; // 'for'

        self.stream.expect(TokenKind::LBrace)?;
        let var = self.parse_ident()?;
        self.stream.expect(TokenKind::In)?;
        let iterable = self.parse_expr()?;
        self.stream.expect(TokenKind::RBrace)?;

        self.stream.expect(TokenKind::LBrace)?;
        let body = self.parse_block_children()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiNode::For {
            var,
            iterable,
            body,
            span: Span::merge(start, end),
        })
    }

    /// `case <label> { ...children... }` -- the block-mode arm of a
    /// `match{scrutinee}` node, replacing the old XML dialect's `<case
    /// label>...</case>`. Produces the exact same AST shape that old
    /// syntax did: a node carrying a synthetic `UiAttribute{name: "case",
    /// value: Ident(label)}` (see
    /// `tint-runtime/src/ui/builder/helpers.rs`'s `find_case_label`/
    /// `case_label_matches`, and `build_container`'s match{} case-scanning
    /// in `tint-runtime/src/ui/builder/nodes.rs`, neither of which had to
    /// change to accept this -- they already just look for a `case`
    /// attribute on any child, regardless of which syntax produced it).
    /// `_` is the wildcard arm; it lexes as its own `TokenKind::Underscore`
    /// (same token the `match` expression's wildcard pattern uses in Logic
    /// Mode), not `Ident`, hence the explicit check for both above.
    pub(crate) fn parse_block_case_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.next().span; // 'case'

        let label_tok = self.stream.next();
        let label = label_tok.lexeme.clone();
        let label_span = label_tok.span;

        self.stream.expect(TokenKind::LBrace)?;
        let children = self.parse_block_children()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;

        Ok(UiNode::BlockElement {
            name: "case".to_string(),
            attributes: vec![UiAttribute {
                name: "case".to_string(),
                value: UiAttrValue::Ident(label),
                span: label_span,
            }],
            modifiers: vec![],
            children,
            span: Span::merge(start, end),
        })
    }

    pub(crate) fn parse_theme_node(&mut self) -> PResult<UiNode> {
        let start = self.stream.expect(TokenKind::Ident)?.span;
        self.stream.expect(TokenKind::PathSep)?;
        let name = self.stream.expect(TokenKind::Ident)?.lexeme;
        self.stream.expect(TokenKind::LBrace)?;
        let children = self.parse_block_children()?;
        let end = self.stream.expect(TokenKind::RBrace)?.span;
        Ok(UiNode::Theme {
            name,
            children,
            span: Span::merge(start, end),
        })
    }
}
