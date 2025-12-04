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
                Err(ParserError::Message {
                    msg: "`module` parsing not implemented yet".into(),
                    span: self.stream.peek().span,
                })
            }

            TokenKind::Use => {
                Err(ParserError::Message {
                    msg: "`use` import syntax not implemented yet".into(),
                    span: self.stream.peek().span,
                })
            }

            TokenKind::Impl => {
                Err(ParserError::Message {
                    msg: "`impl` blocks not implemented yet".into(),
                    span: self.stream.peek().span,
                })
            }

            // FALLBACK ERROR
            other => Err(ParserError::Message {
                msg: format!("Unexpected token {:?}, expected fn/ui/struct/enum", other),
                span: self.stream.peek().span,
            }),
        }
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
            // 1) parse name
            let name = self.parse_ident()?;

            // 2) :
            self.stream.expect(TokenKind::Colon)?;

            // 3) type
            let ty = self.parse_type()?;

            // 4) default value — Rune style: { expr }
            let default = if self.stream.consume_if(TokenKind::LBrace) {
                let expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBrace)?;
                seen_default = true;
                Some(expr)
            } else {
                // если уже были default-параметры → нельзя дальше без них
                if seen_default {
                    return Err(ParserError::Message {
                        msg: "Cannot mix parameters with and without default values".into(),
                        span: self.stream.peek().span,
                    });
                }
                None
            };

            params.push(Param { name, ty, default });

            // comma?
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }
    
}
