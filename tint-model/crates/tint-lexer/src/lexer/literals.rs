use super::{is_ident_continue, Lexer};
use crate::{Token, TokenKind};
use tint_ast::{Position, Span};

impl Lexer<'_> {
    pub(super) fn lex_string(&mut self, start: Position) -> Token {
        self.bump();
        let mut value = String::new();

        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(escaped) = self.bump() {
                        value.push(escaped);
                    }
                }
                _ => value.push(c),
            }
        }

        Token::new(TokenKind::String, Span::new(start, self.position()), value)
    }

    pub(super) fn lex_number(&mut self, start: Position) -> Token {
        let mut value = String::new();

        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            value.push(self.bump().expect("peeked digit must exist"));
        }

        if self.fraction_follows() {
            value.push(self.bump().expect("fraction dot must exist"));

            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                value.push(self.bump().expect("peeked digit must exist"));
            }
        }

        Token::new(TokenKind::Number, Span::new(start, self.position()), value)
    }

    // `#RGB` / `#RRGGBB` / `#RRGGBBAA` hex colors (e.g. `background::#6c5ce7`).
    // Lexed as a single Ident-kind token whose lexeme includes the `#`,
    // rather than a dedicated token kind -- to the parser/AST a hex color
    // is just another opaque style string, exactly like a named color
    // ("white", "black", ...) already is; only the UI style resolver
    // downstream (tint-runtime/src/ui/style.rs) cares what it means.
    // Without this, `#` fell through to the generic "unknown character"
    // case and `#6c5ce7` split into three bogus tokens (`#`, `6`, `c5ce7`).
    pub(super) fn lex_color(&mut self, start: Position) -> Token {
        let mut value = String::new();
        value.push(self.bump().expect("'#' must exist"));

        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            value.push(self.bump().expect("peeked hex digit must exist"));
        }

        Token::new(TokenKind::Ident, Span::new(start, self.position()), value)
    }

    pub(super) fn lex_ident(&mut self, start: Position) -> Token {
        let mut value = String::new();

        while self.peek().is_some_and(is_ident_continue) {
            value.push(self.bump().expect("peeked identifier character must exist"));
        }

        Token::new(
            keyword_kind(&value).unwrap_or(TokenKind::Ident),
            Span::new(start, self.position()),
            value,
        )
    }

    fn fraction_follows(&self) -> bool {
        matches!(self.peek2(), Some(('.', next)) if next.is_ascii_digit())
    }
}

fn keyword_kind(value: &str) -> Option<TokenKind> {
    Some(match value {
        "fn" => TokenKind::Fn,
        "return" => TokenKind::Return,
        "let" => TokenKind::Let,
        "if" => TokenKind::If,
        "else" => TokenKind::Else,
        "match" => TokenKind::Match,
        "for" => TokenKind::For,
        "while" => TokenKind::While,
        "loop" => TokenKind::Loop,
        "break" => TokenKind::Break,
        "continue" => TokenKind::Continue,
        "in" => TokenKind::In,
        "where" => TokenKind::Where,
        "true" => TokenKind::True,
        "false" => TokenKind::False,
        "_" => TokenKind::Underscore,
        "async" => TokenKind::Async,
        "await" => TokenKind::Await,
        "try" => TokenKind::Try,
        "struct" => TokenKind::Struct,
        "enum" => TokenKind::Enum,
        "impl" => TokenKind::Impl,
        "self" => TokenKind::SelfKw,
        "space" => TokenKind::Space,
        "kernel" => TokenKind::Kernel,
        "borrow" => TokenKind::Borrow,
        "immut" => TokenKind::Immut,
        "move" => TokenKind::Move,
        "clone" => TokenKind::Clone,
        "ui" => TokenKind::Ui,
        "state" => TokenKind::State,
        "signal" => TokenKind::Signal,
        "computed" => TokenKind::Computed,
        "map" => TokenKind::MapLit,
        "type" => TokenKind::Type,
        "tint2d" => TokenKind::Tint2d,
        "module" | "mod" => TokenKind::Module,
        "export" => TokenKind::Export,
        "use" => TokenKind::Use,
        "import" => TokenKind::Import,
        "as" => TokenKind::As,
        _ => return None,
    })
}
