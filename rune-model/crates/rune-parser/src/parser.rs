// rune-parser/parser.rs

use rune_lexer::{Token, TokenKind};
use crate::{ token_stream::TokenStream, error::* };
use rune_ast::*;

pub struct Parser {
    pub(crate) stream: TokenStream,
    pub(crate) in_pattern: bool,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { stream: TokenStream::new(tokens), in_pattern: false}
    }

    // PROGRAM
    pub fn parse_program(&mut self) -> PResult<Program> {
        let mut items = Vec::new();

        while !self.stream.at_end() {
            let item = self.parse_item()?;
            items.push(item);
        }

        Ok(Program { items })
    }

    pub fn parse_item(&mut self) -> PResult<Item> {
        match self.stream.peek_kind() {

            // FUNCTION or ASYNC FUNCTION
            TokenKind::Fn | TokenKind::Async => {
                let decl = self.parse_fn_decl()?;
                Ok(Item::Fn(decl))
            }

            // UI "fn" (different keyword)
            TokenKind::Ui => {
                let u = self.parse_ui_fn()?;
                Ok(Item::UiFn(u))
            }

            // STRUCT DECL
            TokenKind::Struct => {
                let s = self.parse_struct()?;
                Ok(Item::Struct(s))
            }

            // ENUM DECL
            TokenKind::Enum => {
                let e = self.parse_enum()?;
                Ok(Item::Enum(e))
            }

            // LATER SUPPORT:
            TokenKind::Module => {
                let m = self.parse_mod()?;
                return Ok(Item::Mod(m));
            }

            TokenKind::Use => {
                let u = self.parse_use()?;
                return Ok(Item::Use(u));
            }

            TokenKind::Impl => {
                Err(ParserError::Message {
                    msg: "`impl` blocks not implemented yet".into(),
                    span: self.stream.peek().span,
                })
            }

            TokenKind::Let => {
                let start = self.stream.next().span;
                let stmt = self.parse_let_stmt(start)?;
                Ok(Item::GlobalLet(stmt))
            }

            // FALLBACK ERROR
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
    Ok(UseDecl { path, span: Span::merge(start, end) })
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
    Ok(ModDecl { name, items, span: Span::merge(start, end) })
}


    pub fn new_expr_only(expr_src: String, _span: Span) -> Self {
        use rune_lexer::Lexer;

        let mut lexer = Lexer::new(&expr_src);
        let tokens = rune_lexer::collect_tokens(&mut lexer);

        Parser::new(tokens)
    }

    /// Parse function parameters:  (name: Type, name: Type, ...)
   pub(crate) fn parse_params(&mut self) -> PResult<Vec<Param>> {
        let mut params = Vec::new();
        let mut seen_default = false;

        // empty params: ()
        if self.stream.check(TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            // PATTERN (а не имя)
            self.in_pattern = true;
            let pattern = self.parse_pattern()?;    // <-- ВАЖНО
            self.in_pattern = false;

            // optional ": type"
            let ty = if self.stream.consume_if(TokenKind::Colon) {
                Some(self.parse_type()?)
            } else {
                None
            };

            // default: {expr}
            let default = if self.stream.consume_if(TokenKind::LBrace) {
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                seen_default = true;
                Some(expr)
            } else {
                if seen_default && ty.is_none() {
                    return Err(ParserError::Message {
                        msg: "Cannot mix parameters with and without default values".into(),
                        span: self.stream.peek().span,
                    });
                }
                None
            };

            params.push(Param { pattern, ty, default });

            // comma?
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }
    
}
