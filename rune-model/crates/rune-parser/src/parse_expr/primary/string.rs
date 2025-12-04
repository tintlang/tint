// rune-parser/parse_expr/primary/string.rs

use crate::{Parser, error::*};
use rune_ast::{Expr, Span};

impl Parser {
    pub(crate) fn parse_string_or_interpolated(&mut self) -> PResult<Expr> {
        let t = self.stream.next();

        if !t.lexeme.contains('{') {
            return Ok(Expr::String(t.lexeme, t.span));
        }

        self.parse_interpolated_string(t.lexeme, t.span)
    }

    pub(crate) fn parse_interpolated_string(&mut self, raw: String, span: Span) -> PResult<Expr> {
        use rune_ast::StringPart::{Text, Expr as PartExpr};

        let mut parts = Vec::new();
        let chars: Vec<char> = raw.chars().collect();

        let mut i = 0;
        let mut buf = String::new();

        while i < chars.len() {
            match chars[i] {
                '{' => {
                    if !buf.is_empty() {
                        parts.push(Text(buf.clone()));
                        buf.clear();
                    }

                    i += 1;
                    let start = i;
                    let mut depth = 1;

                    while i < chars.len() && depth > 0 {
                        if chars[i] == '{' { depth += 1; }
                        if chars[i] == '}' { depth -= 1; }
                        i += 1;
                    }

                    let expr_src = &raw[start..i-1];

                    let mut parser = Parser::new_expr_only(expr_src.into(), span);
                    let expr = parser.parse_expr()?;

                    parts.push(PartExpr(expr));
                }

                c => {
                    buf.push(c);
                    i += 1;
                }
            }
        }

        if !buf.is_empty() {
            parts.push(Text(buf));
        }

        Ok(Expr::InterpolatedString { parts, span })
    }
}
