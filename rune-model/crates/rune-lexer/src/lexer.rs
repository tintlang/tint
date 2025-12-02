// crates/lexer/src/lexer.rs
use crate::{Token, TokenKind};
use rune_ast::{Span, Position};

pub struct Lexer<'a> {
    src: &'a str,
    chars: std::str::Chars<'a>,
    pos: usize,
    line: usize,
    column: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            chars: src.chars(),
            pos: 0,
            line: 1,
            column: 1,
        }
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.next()?;

        if c == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }

        self.pos += c.len_utf8();
        Some(c)
    }

    fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    fn peek2(&self) -> Option<(char, char)> {
        let mut clone = self.chars.clone();
        let a = clone.next()?;
        let b = clone.next()?;
        Some((a, b))
    }

    fn peek3(&self) -> Option<(char, char, char)> {
        let mut clone = self.chars.clone();
        Some((clone.next()?, clone.next()?, clone.next()?))
    }

    fn position(&self) -> Position {
        Position::new(self.pos, self.line, self.column)
    }

    // MAIN
    pub fn next_token(&mut self) -> Token {
        self.skip_ws_and_comments();

        let start = self.position();
        let Some(c) = self.peek() else {
            return Token::new(TokenKind::Eof, Span::new(start, start), "");
        };

        // 1) Multi-char operators (longest first)
        if let Some(tok) = self.try_multi(start) {
            return tok;
        }

        // 2) Single char tokens
        match c {
            '{' => return self.single(TokenKind::LBrace),
            '}' => return self.single(TokenKind::RBrace),
            '(' => return self.single(TokenKind::LParen),
            ')' => return self.single(TokenKind::RParen),
            '<' => return self.single(TokenKind::LAngle),
            '>' => return self.single(TokenKind::RAngle),
            '[' => return self.single(TokenKind::LBracket),
            ']' => return self.single(TokenKind::RBracket),
            ',' => return self.single(TokenKind::Comma),
            ':' => return self.single(TokenKind::Colon),
            ';' => return self.single(TokenKind::Semicolon),
            '.' => return self.single(TokenKind::Dot),
            '+' => return self.single(TokenKind::Plus),
            '-' => return self.single(TokenKind::Minus),
            '*' => return self.single(TokenKind::Star),
            '/' => return self.single(TokenKind::Slash),
            '%' => return self.single(TokenKind::Percent),
            '!' => return self.single(TokenKind::Bang),
            '|' => return self.single(TokenKind::Pipe),

            '"' => return self.lex_string(start),

            c if c.is_ascii_digit() => return self.lex_number(start),
            c if is_ident_start(c) => return self.lex_ident(start),

            _ => {
                self.bump();
                return Token::new(TokenKind::Ident, Span::new(start, self.position()), c.to_string());
            }
        }
    }

    fn single(&mut self, kind: TokenKind) -> Token {
        let start = self.position();
        let c = self.bump().unwrap();
        Token::new(kind, Span::new(start, self.position()), c.to_string())
    }

    // WHITESPACE + COMMENTS
    fn skip_ws_and_comments(&mut self) {
        loop {
            let Some(c) = self.peek() else { return };

            if c.is_whitespace() {
                self.bump();
                continue;
            }

            if c == '/' {
                let mut clone = self.chars.clone();
                clone.next();
                if clone.next() == Some('/') {
                    // line comment
                    self.bump();
                    self.bump();
                    while let Some(c) = self.peek() {
                        if c == '\n' { break }
                        self.bump();
                    }
                    continue;
                }
            }

            return;
        }
    }

    // MULTI-CHAR TOKENS
    fn try_multi(&mut self, start: Position) -> Option<Token> {
        if let Some((a,b,c)) = self.peek3() {
            if a == '.' && b == '.' && c == '.' {
                self.bump(); self.bump(); self.bump();
                return Some(Token::new(TokenKind::DotDotDot, Span::new(start, self.position()), "..."));
            }
        }

        if let Some((a,b)) = self.peek2() {
            return match (a,b) {
                (':', ':') => Some(self.eat2(start, TokenKind::PathSep, "::")),

                ('=', '=') => Some(self.eat2(start, TokenKind::EqEq, "==")),
                ('!', '=') => Some(self.eat2(start, TokenKind::NotEq, "!=")),
                ('<', '=') => Some(self.eat2(start, TokenKind::LessEq, "<=")),
                ('>', '=') => Some(self.eat2(start, TokenKind::GreaterEq, ">=")),
                ('&', '&') => Some(self.eat2(start, TokenKind::AndAnd, "&&")),
                ('|', '|') => Some(self.eat2(start, TokenKind::OrOr, "||")),

                ('+', '=') => Some(self.eat2(start, TokenKind::PlusEq, "+=")),
                ('-', '=') => Some(self.eat2(start, TokenKind::MinusEq, "-=")),
                ('*', '=') => Some(self.eat2(start, TokenKind::StarEq, "*=")),
                ('/', '=') => Some(self.eat2(start, TokenKind::SlashEq, "/=")),

                ('-', '>') => Some(self.eat2(start, TokenKind::Arrow, "->")),
                ('=', '>') => Some(self.eat2(start, TokenKind::FatArrow, "=>")),

                ('<', '/') => Some(self.eat2(start, TokenKind::AngleSlash, "</")),
                ('/', '>') => Some(self.eat2(start, TokenKind::SlashAngle, "/>")),

                ('.', '.') => Some(self.eat2(start, TokenKind::DotDot, "..")),

                _ => None,
            };
        }

        None
    }

    fn eat2(&mut self, start: Position, kind: TokenKind, lexeme: &str) -> Token {
        self.bump();
        self.bump();
        Token::new(kind, Span::new(start, self.position()), lexeme)
    }

    // STRING
    fn lex_string(&mut self, start: Position) -> Token {
        self.bump(); // open quote
        let mut s = String::new();

        while let Some(c) = self.bump() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(next) = self.bump() {
                        s.push(next);
                    }
                }
                _ => s.push(c),
            }
        }

        Token::new(TokenKind::String, Span::new(start, self.position()), s)
    }

    // NUMBER
    fn lex_number(&mut self, start: Position) -> Token {
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                s.push(self.bump().unwrap());
            } else {
                break;
            }
        }

        Token::new(TokenKind::Number, Span::new(start, self.position()), s)
    }

    // IDENT / KEYWORD
    fn lex_ident(&mut self, start: Position) -> Token {
        let mut s = String::new();

        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                s.push(self.bump().unwrap());
            } else {
                break;
            }
        }

        let kind = match s.as_str() {
            "fn" => TokenKind::Fn,
            "ui" => TokenKind::Ui,
            "return" => TokenKind::Return,
            "let" => TokenKind::Let,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "match" => TokenKind::Match,
            "for" => TokenKind::For,
            "in" => TokenKind::In,
            "where" => TokenKind::Where,
            "while" => TokenKind::While,
            "loop" => TokenKind::Loop,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "try" => TokenKind::Try,

            "enum" => TokenKind::Enum,
            "struct" => TokenKind::Struct,
            "impl" => TokenKind::Impl,
            "self" => TokenKind::SelfKw,

            "borrow" => TokenKind::Borrow,
            "immut" => TokenKind::Immut,
            "move" => TokenKind::Move,
            "clone" => TokenKind::Clone,

            "state" => TokenKind::State,
            "signal" => TokenKind::Signal,
            "computed" => TokenKind::Computed,
            "rune2d" => TokenKind::Rune2d,

            "module" => TokenKind::Module,
            "export" => TokenKind::Export,
            "use" => TokenKind::Use,

            _ => TokenKind::Ident,
        };

        Token::new(kind, Span::new(start, self.position()), s)
    }
}

// HELPERS
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
