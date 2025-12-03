// rune-parser/parser.rs

use rune_lexer::{Token, TokenKind};
use crate::{ token_stream::TokenStream, error::* };
use rune_ast::*;

pub struct Parser {
    pub(crate) stream: TokenStream,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self { stream: TokenStream::new(tokens) }
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

    // ITEM DISPATCHER (минимальный: fn, ui fn, struct, enum)
    pub fn parse_item(&mut self) -> PResult<Item> {
        match self.stream.peek_kind() {

            // FUNCTION
            TokenKind::Fn => {
                let f = self.parse_fn_decl()?;
                Ok(Item::Fn(f))
            }

            // async fn 
            TokenKind::Async => {
                return Err(ParserError::Message {
                    msg: "async not implemented yet".into(),
                    span: self.stream.peek().span,
                });
            }

            // UI FUNCTION
            TokenKind::Ui => {
                let ui = self.parse_ui_fn()?;
                Ok(Item::UiFn(ui))
            }

            // STRUCT
            TokenKind::Struct => {
                let st = self.parse_struct()?;
                Ok(Item::Struct(st))
            }

            // ENUM
            TokenKind::Enum => {
                let en = self.parse_enum()?;
                Ok(Item::Enum(en))
            }

            // otherwise -> error
            other => {
                Err(ParserError::Message {
                    msg: format!(
                        "Unexpected token {:?}, expected fn / ui fn / struct / enum",
                        other
                    ),
                    span: self.stream.peek().span,
                })
            }
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

        // empty:  ()
        if self.stream.check(TokenKind::RParen) {
            return Ok(params);
        }

        loop {
            // param name
            let name = self.parse_ident()?;

            // :
            self.stream.expect(TokenKind::Colon)?;

            // type
            let ty = self.parse_type()?;     // ✔ already exists in parser

            params.push(Param { name, ty });

            // comma?
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }
        }

        Ok(params)
    }
    
}
