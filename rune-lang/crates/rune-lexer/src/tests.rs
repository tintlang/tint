#[cfg(test)]
mod tests {
    use crate::*;

    #[test]
    fn lex_simple_ident() {
        let mut lx = Lexer::new("hello");
        let t = lx.next_token();
        assert_eq!(t.kind, TokenKind::Ident);
    }
}
