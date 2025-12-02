use crate::{Token, TokenKind};
use rune_ast::{Span, Position};

pub struct Lexer<'a> {
    src: &'a str,
    chars: std::str::Chars<'a>,
    pos: usize,     // byte offset
    line: usize,    // 1-based
    column: usize,  // 1-based
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

    fn position(&self) -> Position {
        Position::new(self.pos, self.line, self.column)
    }

    // MAIN ENTRY
    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace_and_comments();

        let start_pos = self.position();
        let Some(c) = self.peek() else {
            return Token::new(TokenKind::Eof, Span::new(start_pos, start_pos), "");
        };

        // multi-char operators first
        if let Some(tok) = self.try_multi_char(start_pos) {
            return tok;
        }

        // simple tokens
        match c {
            '{' => return self.single(TokenKind::LBrace),
            '}' => return self.single(TokenKind::RBrace),
            '(' => return self.single(TokenKind::LParen),
            ')' => return self.single(TokenKind::RParen),
            '<' => return self.single(TokenKind::LAngle),
            '>' => return self.single(TokenKind::RAngle),
            ':' => return self.single(TokenKind::Colon),
            ',' => return self.single(TokenKind::Comma),
            '=' => return self.single(TokenKind::Eq),
            ';' => return self.single(TokenKind::Semicolon), 

            '+' => return self.single(TokenKind::Plus),
            '-' => return self.single(TokenKind::Minus),
            '*' => return self.single(TokenKind::Star),
            '/' => return self.single(TokenKind::Slash),

            '"' => return self.lex_string(start_pos),

            c if c.is_ascii_digit() => return self.lex_number(start_pos),
            c if is_ident_start(c) => return self.lex_ident_or_keyword(start_pos),

            _ => {
                self.bump();
                return Token::new(TokenKind::Ident, Span::new(start_pos, self.position()), c.to_string());
            }
        }
    }

    // HELPERS
    fn single(&mut self, kind: TokenKind) -> Token {
        let start = self.position();
        let c = self.bump().unwrap();
        let end = self.position();
        Token::new(kind, Span::new(start, end), c.to_string())
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let Some(c) = self.peek() else { return; };

            // whitespace
            if c.is_whitespace() {
                self.bump();
                continue;
            }

            // line comment //
            if c == '/' {
                let mut clone = self.chars.clone();
                clone.next();
                if clone.next() == Some('/') {
                    self.bump(); // '/'
                    self.bump(); // '/'
                    while let Some(ch) = self.peek() {
                        if ch == '\n' { break; }
                        self.bump();
                    }
                    continue;
                }
            }
            return;
        }
    }

    // MULTI-CHAR OPERATORS
    fn try_multi_char(&mut self, start_pos: Position) -> Option<Token> {
        let mut clone = self.chars.clone();
        let first = clone.next()?;
        let second = clone.next();

        match (first, second) {
            ('-', Some('>')) => return Some(self.eat_two(start_pos, TokenKind::Arrow, "->")),
            ('=', Some('>')) => return Some(self.eat_two(start_pos, TokenKind::FatArrow, "=>")),
            ('.', Some('.')) => return Some(self.eat_two(start_pos, TokenKind::DotDot, "..")),
            ('<', Some('/')) => return Some(self.eat_two(start_pos, TokenKind::AngleSlash, "</")),
            ('/', Some('>')) => return Some(self.eat_two(start_pos, TokenKind::SlashAngle, "/>")),
            _ => None,
        }
    }

    fn eat_two(&mut self, start: Position, kind: TokenKind, lexeme: &str) -> Token {
        self.bump(); // first
        self.bump(); // second
        let end = self.position();
        Token::new(kind, Span::new(start, end), lexeme)
    }

    // STRINGS
    fn lex_string(&mut self, start_pos: Position) -> Token {
        self.bump(); // open quote
        let mut value = String::new();

        while let Some(c) = self.bump() {
            match c {
                '"' => break,      // close string
                '\\' => {
                    if let Some(next) = self.bump() {
                        value.push(next);
                    }
                }
                _ => value.push(c),
            }
        }

        let end_pos = self.position();
        Token::new(TokenKind::String, Span::new(start_pos, end_pos), value)
    }

    // NUMBER LITERAL
    fn lex_number(&mut self, start_pos: Position) -> Token {
        let mut value = String::new();

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() || c == '.' {
                value.push(self.bump().unwrap());
            } else {
                break;
            }
        }

        let end_pos = self.position();
        Token::new(TokenKind::Number, Span::new(start_pos, end_pos), value)
    }

    // IDENTIFIER / KEYWORD
    fn lex_ident_or_keyword(&mut self, start_pos: Position) -> Token {
        let mut value = String::new();

        while let Some(c) = self.peek() {
            if is_ident_continue(c) {
                value.push(self.bump().unwrap());
            } else {
                break;
            }
        }

        let kind = match value.as_str() {
            "fn" => TokenKind::Fn,
            "ui" => TokenKind::Ui,
            "let" => TokenKind::Let,
            "state" => TokenKind::State,
            "signal" => TokenKind::Signal,
            "computed" => TokenKind::Computed,
            "match" => TokenKind::Match,
            "for" => TokenKind::For,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "enum" => TokenKind::Enum,
            "struct" => TokenKind::Struct,
            "export" => TokenKind::Export,
            "module" => TokenKind::Module,
            "borrow" => TokenKind::Borrow,
            "immut" => TokenKind::Immut,
            "try" => TokenKind::Try,
            "move" => TokenKind::Move,
            "clone" => TokenKind::Clone,
            _ => TokenKind::Ident,
        };

        let end_pos = self.position();
        Token::new(kind, Span::new(start_pos, end_pos), value)
    }
}

// HELPERS
fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_' 
}

fn is_ident_continue(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_' || c == '-'
}
