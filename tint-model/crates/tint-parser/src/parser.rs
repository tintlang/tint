use crate::{error::*, symbols::Symbols, token_stream::TokenStream};
use tint_ast::*;
use tint_lexer::{Token, TokenKind};

pub struct Parser {
    pub(crate) stream: TokenStream,
    pub symbols: Symbols,
    pub(crate) in_pattern: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            stream: TokenStream::new(tokens),
            symbols: Symbols::default(),
            in_pattern: false,
        }
    }

    fn clone_for_first_pass(&self) -> Self {
        Self {
            stream: self.stream.clone_with_reset(),
            symbols: Symbols::default(),
            in_pattern: false,
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
            items.push(self.parse_item()?);
        }

        Ok(Program { globals, items })
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

            TokenKind::Use => {
                let u = self.parse_use()?;
                return Ok(Item::Use(u));
            }

            TokenKind::Import => return self.parse_import(),

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

    pub(crate) fn parse_use(&mut self) -> PResult<UseDecl> {
        let start = self.stream.expect(TokenKind::Use)?.span;

        let mut path = Vec::new();
        let first = self.parse_ident()?;
        path.push(first);

        while self.stream.consume_if(TokenKind::PathSep) {
            let seg = self.parse_ident()?;
            path.push(seg);
        }

        self.stream.expect(TokenKind::Semicolon)?;

        let end = self.stream.last_span();
        Ok(UseDecl {
            path,
            span: Span::merge(start, end),
        })
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

        self.stream.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();
        while !self.stream.consume_if(TokenKind::RBrace) {
            items.push(self.parse_item()?);
        }

        let end = self.stream.last_span();
        Ok(ModDecl {
            name,
            items,
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
