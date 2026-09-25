use crate::Parser;
use crate::error::PResult;
use rune_lexer::TokenKind;
use rune_ast::Item;
use rune_ast::SpaceKind;
use rune_ast::Span;
use rune_ast::SpaceDecl;

impl Parser {
    pub fn parse_space(&mut self) -> PResult<SpaceDecl> {
        let start = self.stream.expect(TokenKind::Space)?.span;

        // parse name: space Xxx
        let name_tok = self.stream.expect_ident()?;
        let mut name = name_tok.lexeme.clone();
        let mut kind = SpaceKind::Auto;

        // space.all Xxx
        if name == "all" {
            self.stream.expect(TokenKind::Dot)?;
            let real_name_tok = self.stream.expect_ident()?;
            name = real_name_tok.lexeme.clone();
            kind = SpaceKind::All;
        }

        self.parse_space_body(name, kind, start)
    }

    fn parse_space_body(
        &mut self,
        name: String,
        kind: SpaceKind,
        start: Span
    ) -> PResult<SpaceDecl> {

        self.stream.expect(TokenKind::LBrace)?;

        let mut items = Vec::new();
        let mut detected_kind = kind;

        while !self.stream.consume_if(TokenKind::RBrace) {
            let item = self.parse_item()?;

            if detected_kind == SpaceKind::Auto {
                detected_kind = Self::determine_space_kind(&item);
            } else {
                self.validate_item_in_space(&item, detected_kind)?;
            }

            items.push(item);
        }

        let end = self.stream.last_span();

        Ok(SpaceDecl {
            name,
            kind: detected_kind,
            items,
            span: Span::merge(start, end),
        })
    }


    fn validate_item_in_space(&self, item: &Item, kind: SpaceKind) -> PResult<()> {
        use SpaceKind::*;

        match kind {
            All => Ok(()),

            GPU => match item {
                Item::Kernel(_) => Ok(()),
                _ => self.error("Only `kernel` is allowed in GPU-space"),
            },

            UI => match item {
                Item::UiFn(_) => Ok(()),
                _ => self.error("Only `ui fn` is allowed in UI-space"),
            },

            Logic => match item {
                Item::Fn(_)
                | Item::Struct(_)
                | Item::Enum(_)
                | Item::Impl(_)
                => Ok(()),

                _ => self.error("Only fn, struct, enum, impl allowed in logic space"),
            },

            Auto => unreachable!(),
        }
    }


    fn determine_space_kind(item: &Item) -> SpaceKind {
        match item {
            Item::Kernel(_) => SpaceKind::GPU,
            Item::UiFn(_)   => SpaceKind::UI,

            Item::Fn(_)
            | Item::Struct(_)
            | Item::Enum(_)
            | Item::Impl(_)
            | Item::TypeAlias(_)
            | Item::Use(_)
            | Item::Mod(_)
            | Item::GlobalLet(_)
            | Item::ExportFn(_, _)
            | Item::ExportStruct(_)
            | Item::ExportEnum(_)
            => SpaceKind::Logic,

            // Defensive fallback for an otherwise unreachable branch.
            _ => SpaceKind::Logic,
        }
    }
}
