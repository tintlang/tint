use crate::Parser;
use crate::error::PResult;
use rune_ast::Item;
use rune_lexer::TokenKind;
use rune_ast::Span;

impl Parser {
    pub(crate) fn parse_export_item(&mut self) -> PResult<Item> {
        let start = self.stream.expect(TokenKind::Export)?.span;

        match self.stream.peek_kind() {
            TokenKind::Fn => {
                let fn_decl = self.parse_fn_decl()?;
                let span = Span::merge(start, fn_decl.span);
                Ok(Item::ExportFn(fn_decl, span))
            }
            TokenKind::Struct => {
                let s = self.parse_struct()?;
                Ok(Item::ExportStruct(s))
            }
            TokenKind::Enum => {
                let e = self.parse_enum()?;
                Ok(Item::ExportEnum(e))
            }
            _ => self.stream.error_here("Expected fn/struct/enum after `export`"),
        }
    }
}
