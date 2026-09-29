use crate::{error::*, Parser};
use tint_ast::{Expr, Span};

impl Parser {
    pub(crate) fn parse_string_or_interpolated(&mut self) -> PResult<Expr> {
        let t = self.stream.next();

        if !t.lexeme.contains('{') {
            return Ok(Expr::String(t.lexeme, t.span));
        }

        self.parse_interpolated_string(t.lexeme, t.span)
    }

    /// Index just past the `}` closing an interpolation whose body starts at
    /// `i`. Braces inside a nested string literal don't count.
    pub(crate) fn interpolation_end(chars: &[char], mut i: usize) -> usize {
        let mut depth = 1;
        while i < chars.len() && depth > 0 {
            match chars[i] {
                '"' => {
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        if chars[i] == '\\' {
                            i += 1;
                        }
                        i += 1;
                    }
                }
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            i += 1;
        }
        i.min(chars.len())
    }

    pub(crate) fn parse_interpolated_string(&mut self, raw: String, span: Span) -> PResult<Expr> {
        use tint_ast::StringPart::{Expr as PartExpr, Text};

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
                    i = Self::interpolation_end(&chars, i);
                    let end = if i > start && chars[i - 1] == '}' {
                        i - 1
                    } else {
                        i
                    };
                    let expr_src: String = chars[start..end].iter().collect();

                    let mut parser = Parser::new_expr_only(expr_src, span);
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
