use super::Lexer;
use crate::{Token, TokenKind};
use rune_ast::{Position, Span};

impl Lexer<'_> {
    pub(super) fn lex_operator(&mut self, start: Position) -> Option<Token> {
        if self.peek3() == Some(('.', '.', '.')) {
            self.bump();
            self.bump();
            self.bump();
            return Some(Token::new(
                TokenKind::DotDotDot,
                Span::new(start, self.position()),
                "...",
            ));
        }

        let (a, b) = self.peek2()?;
        let (kind, lexeme) = match (a, b) {
            (':', ':') => (TokenKind::PathSep, "::"),
            ('=', '=') => (TokenKind::EqEq, "=="),
            ('!', '=') => (TokenKind::NotEq, "!="),
            ('<', '=') => (TokenKind::LessEq, "<="),
            ('>', '=') => (TokenKind::GreaterEq, ">="),
            ('&', '&') => (TokenKind::AndAnd, "&&"),
            ('|', '|') => (TokenKind::OrOr, "||"),
            ('+', '=') => (TokenKind::PlusEq, "+="),
            ('-', '=') => (TokenKind::MinusEq, "-="),
            ('*', '=') => (TokenKind::StarEq, "*="),
            ('/', '=') => (TokenKind::SlashEq, "/="),
            ('-', '>') => (TokenKind::Arrow, "->"),
            ('=', '>') => (TokenKind::FatArrow, "=>"),
            ('<', '/') => (TokenKind::AngleSlash, "</"),
            ('/', '>') => (TokenKind::SlashAngle, "/>"),
            ('.', '.') => (TokenKind::DotDot, ".."),
            _ => return None,
        };

        Some(self.eat2(start, kind, lexeme))
    }

    fn eat2(&mut self, start: Position, kind: TokenKind, lexeme: &'static str) -> Token {
        self.bump();
        self.bump();
        Token::new(kind, Span::new(start, self.position()), lexeme)
    }
}
