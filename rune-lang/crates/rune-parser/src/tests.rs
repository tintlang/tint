#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    #[test]
    fn test_parse_empty_program() {
        let mut lexer = Lexer::new("");
        let tokens = collect_tokens(&mut lexer);
        let mut parser = Parser::new(tokens);

        let program = parser.parse_program().unwrap();
        assert!(program.items.is_empty());
    }
}
