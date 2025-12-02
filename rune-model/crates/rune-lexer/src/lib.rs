pub mod lexer;
pub mod token;

pub use lexer::Lexer;
pub use token::{Token, TokenKind};

pub fn collect_tokens(lexer: &mut Lexer) -> Vec<Token> {
    let mut tokens = Vec::new();

    loop {
        let tok = lexer.next_token();
        tokens.push(tok.clone());

        if tok.kind == TokenKind::Eof {
            break;
        }
    }

    tokens
}
