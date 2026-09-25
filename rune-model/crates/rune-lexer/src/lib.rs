pub mod lexer;
pub mod token;

pub use lexer::Lexer;
pub use token::{Token, TokenKind};

pub fn collect_tokens(lexer: &mut Lexer<'_>) -> Vec<Token> {
    let mut tokens = Vec::new();

    loop {
        let token = lexer.next_token();
        let is_eof = token.kind == TokenKind::Eof;
        tokens.push(token);

        if is_eof {
            return tokens;
        }
    }
}
