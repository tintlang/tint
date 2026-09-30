use crate::{error::*, symbols::Symbols, token_stream::TokenStream};
use tint_ast::*;
use tint_lexer::{Token, TokenKind};

pub struct Parser {
    pub(crate) stream: TokenStream,
    pub symbols: Symbols,
    pub(crate) in_pattern: bool,
    pub(crate) recover_ui_indentation: bool,
    pub(crate) ui_block_columns: Vec<usize>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            stream: TokenStream::new(tokens),
            symbols: Symbols::default(),
            in_pattern: false,
            recover_ui_indentation: false,
            ui_block_columns: Vec::new(),
        }
    }

    fn clone_for_first_pass(&self) -> Self {
        Self {
            stream: self.stream.clone_with_reset(),
            symbols: Symbols::default(),
            in_pattern: false,
            recover_ui_indentation: false,
            ui_block_columns: Vec::new(),
        }
    }

    // PROGRAM
    pub fn parse_program(&mut self) -> PResult<Program> {
        // Collect declaration names first so later parsing can disambiguate named calls and types.
        {
            let mut shadow = self.clone_for_first_pass();

            while shadow.stream.peek_kind() != TokenKind::Eof {
                match shadow.stream.peek_kind() {
                    TokenKind::Fn => {
                        shadow.stream.next();
                        while shadow.stream.peek_kind() == TokenKind::LBracket {
                            shadow.skip_attributes()?;
                        }
                        let name = shadow.parse_ident()?;
                        self.symbols.functions.insert(name);
                        shadow.skip_fn_decl()?;
                    }

                    TokenKind::Struct => {
                        shadow.stream.next();
                        let name = shadow.parse_ident()?;
                        self.symbols.types.insert(name);
                        shadow.skip_struct_decl()?;
                    }

                    TokenKind::Enum => {
                        shadow.stream.next();
                        let name = shadow.parse_ident()?;
                        self.symbols.types.insert(name);
                        shadow.skip_enum_decl()?;
                    }

                    _ => {
                        shadow.stream.next();
                    }
                }
            }
        }

        let mut items = Vec::new();

        let mut globals = AttributeList::empty();
        while let Some(a) = self.try_parse_attributes()? {
            globals.extend(a);
        }

        while self.stream.peek_kind() != TokenKind::Eof {
            self.parse_item_into(&mut items)?;
        }

        Ok(Program { globals, items })
    }

    /// Parses one item-position entry and appends whatever it produces to
    /// `items` -- one `Item` for everything except `use`, which can
    /// desugar into several (`use a::{b, c};`). Both item-collecting loops
    /// (`parse_program`'s top level and `parse_mod`'s inline body) call
    /// this instead of `parse_item` directly so a grouped `use` doesn't
    /// need special-casing at either call site.
    fn parse_item_into(&mut self, items: &mut Vec<Item>) -> PResult<()> {
        if self.stream.peek_kind() == TokenKind::Use {
            let uses = self.parse_use()?;
            items.extend(uses.into_iter().map(Item::Use));
            return Ok(());
        }
        items.push(self.parse_item()?);
        Ok(())
    }

    pub fn parse_item(&mut self) -> PResult<Item> {
        match self.stream.peek_kind() {
            TokenKind::Space => {
                let s = self.parse_space()?;
                return Ok(Item::Space(s));
            }

            TokenKind::Impl => self.parse_impl(),

            TokenKind::Type => {
                return self.parse_type_alias();
            }

            TokenKind::Export => self.parse_export_item(),

            TokenKind::Fn | TokenKind::Async => {
                let decl = self.parse_fn_decl()?;
                Ok(Item::Fn(decl))
            }

            TokenKind::Const => self.parse_const_decl(),

            TokenKind::Ui => {
                let u = self.parse_ui_fn()?;
                Ok(Item::UiFn(u))
            }

            TokenKind::Kernel => Ok(Item::Kernel(self.parse_kernel()?)),

            TokenKind::Struct => {
                let decl = self.parse_struct()?;
                Ok(Item::Struct(decl))
            }

            TokenKind::Enum => {
                let decl = self.parse_enum()?;
                Ok(Item::Enum(decl))
            }

            TokenKind::Module => {
                let m = self.parse_mod()?;
                return Ok(Item::Mod(m));
            }

            TokenKind::Use => Err(ParserError::Message {
                msg: "`use` can desugar into more than one item -- call `parse_item_into`, not `parse_item`, wherever a `use` may appear".to_string(),
                span: self.stream.peek().span,
            }),

            TokenKind::Import => return self.parse_import(),

            // `app { title::"...", lang::"..." }`: `app` is a contextual word,
            // not a keyword, so it stays usable as an ordinary identifier.
            TokenKind::Ident
                if self.stream.peek().lexeme == "app"
                    && self.stream.peek_n_kind(1) == TokenKind::LBrace =>
            {
                let start = self.stream.next().span;
                self.stream.expect(TokenKind::LBrace)?;
                let (_, modifiers) = self.parse_modifier_list_block()?;
                let end = self.stream.expect(TokenKind::RBrace)?.span;
                Ok(Item::App(AppDecl {
                    modifiers,
                    span: Span::merge(start, end),
                }))
            }

            TokenKind::Let => {
                let start = self.stream.next().span;
                let stmt = self.parse_let_stmt(start)?;
                Ok(Item::GlobalLet(stmt))
            }

            other => Err(ParserError::Message {
                msg: format!("Unexpected token {:?}, expected fn/ui/struct/enum", other),
                span: self.stream.peek().span,
            }),
        }
    }

    /// `use a::b::c;`, and now also `use a::b as c;` (import under a local
    /// alias), `use a::*;` (wildcard -- every exported item of a module),
    /// and `use a::{b, c as d, e::f};` (grouped, arbitrarily nested, each
    /// sub-item optionally carrying its own `as`/`*`). A single `use`
    /// statement can therefore desugar into more than one `UseDecl` --
    /// that's why this returns a `Vec`, not one -- see the two call sites
    /// (`parse_program`'s item loop and `parse_mod`'s inline-body loop),
    /// which `extend` their item list with `Item::Use(..)` per entry
    /// instead of pushing a single one.
    pub(crate) fn parse_use(&mut self) -> PResult<Vec<UseDecl>> {
        let start = self.stream.expect(TokenKind::Use)?.span;

        let first = self.parse_ident()?;
        let mut out = Vec::new();
        self.parse_use_tail(start, vec![first], &mut out)?;

        self.stream.expect(TokenKind::Semicolon)?;
        Ok(out)
    }

    /// Consumes whatever comes after a `use` path segment has just been
    /// read into `path`: either nothing more (plain path, optional
    /// `as name`), a `::*` wildcard, or a `::{ ... }` group -- recursing
    /// once per group entry so `use a::{b::{c, d as e}, f}` nests as
    /// naturally as the flat cases. Terminates by pushing exactly one
    /// `UseDecl` into `out` per concrete (non-group) path it finds; never
    /// touches the trailing `;`, which only the top-level `parse_use` call
    /// expects (a group entry ends on `,`/`}` instead).
    fn parse_use_tail(
        &mut self,
        start: Span,
        path: Vec<String>,
        out: &mut Vec<UseDecl>,
    ) -> PResult<()> {
        if !self.stream.consume_if(TokenKind::PathSep) {
            let alias = if self.stream.consume_if(TokenKind::As) {
                Some(self.parse_ident()?)
            } else {
                None
            };
            let end = self.stream.last_span();
            out.push(UseDecl {
                path,
                alias,
                wildcard: false,
                span: Span::merge(start, end),
            });
            return Ok(());
        }

        match self.stream.peek_kind() {
            TokenKind::Star => {
                self.stream.next();
                let end = self.stream.last_span();
                out.push(UseDecl {
                    path,
                    alias: None,
                    wildcard: true,
                    span: Span::merge(start, end),
                });
                Ok(())
            }

            TokenKind::LBrace => {
                self.stream.next();
                loop {
                    let mut sub = path.clone();
                    sub.push(self.parse_ident()?);
                    self.parse_use_tail(start, sub, out)?;

                    if self.stream.consume_if(TokenKind::Comma) {
                        if self.stream.peek_kind() == TokenKind::RBrace {
                            break;
                        }
                        continue;
                    }
                    break;
                }
                self.stream.expect(TokenKind::RBrace)?;
                Ok(())
            }

            _ => {
                let mut path = path;
                path.push(self.parse_ident()?);
                self.parse_use_tail(start, path, out)
            }
        }
    }

    pub(crate) fn parse_import(&mut self) -> PResult<Item> {
        let start = self.stream.expect(TokenKind::Import)?.span;
        let path = self.stream.expect(TokenKind::String)?;
        self.stream.expect(TokenKind::Semicolon)?;
        let span = Span::merge(start, self.stream.last_span());
        Ok(Item::Import(ImportDecl {
            path: path.lexeme,
            span,
        }))
    }

    pub(crate) fn parse_mod(&mut self) -> PResult<ModDecl> {
        let start = self.stream.expect(TokenKind::Module)?.span;

        let name = self.parse_ident()?;

        // `mod name;` -- file-backed submodule. No body here; the CLI's
        // module loader resolves and parses `name.tn`/`name/mod.tn` and
        // fills `items` in afterwards (see `ModDecl::external`'s doc
        // comment). The parser stays filesystem-free.
        if self.stream.consume_if(TokenKind::Semicolon) {
            let end = self.stream.last_span();
            return Ok(ModDecl {
                name,
                items: Vec::new(),
                external: true,
                span: Span::merge(start, end),
            });
        }

        self.stream.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();
        while !self.stream.consume_if(TokenKind::RBrace) {
            self.parse_item_into(&mut items)?;
        }

        let end = self.stream.last_span();
        Ok(ModDecl {
            name,
            items,
            external: false,
            span: Span::merge(start, end),
        })
    }

    pub fn new_expr_only(expr_src: String, _span: Span) -> Self {
        use tint_lexer::Lexer;

        let mut lexer = Lexer::new(&expr_src);
        let tokens = tint_lexer::collect_tokens(&mut lexer);

        Parser::new(tokens)
    }

    /// Parses function parameters, including destructuring patterns and brace defaults.
    pub(crate) fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();

        // empty params: ()
        if self.stream.check(TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            self.in_pattern = true;
            let pattern = self.parse_pattern()?;
            self.in_pattern = false;

            let ty = if self.stream.consume_if(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };

            let default = if self.stream.consume_if(TokenKind::LBrace) {
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                match &pattern {
                    Pattern::Ident(_, _) => Some(DefaultValue::Single(expr)),
                    _ => Some(DefaultValue::Broadcast(expr)),
                }
            } else {
                None
            };

            params.push(Param {
                pattern,
                ty,
                default,
            });

            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }

    pub fn error<T>(&self, msg: impl Into<String>) -> PResult<T> {
        let span = self.stream.peek().span;
        Err(crate::error::ParserError::Message {
            msg: msg.into(),
            span,
        })
    }
}
