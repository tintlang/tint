use super::*;

impl Parser {
    pub(crate) fn parse_interpolated_text(&mut self, raw: String, span: Span) -> PResult<UiText> {
        let mut parts = Vec::new();
        let mut buf = String::new();

        let chars: Vec<char> = raw.chars().collect();
        let mut i = 0;

        while i < chars.len() {
            if chars[i] == '{' {
                if !buf.is_empty() {
                    parts.push(UiTextPart::Literal(buf.clone(), span));
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
                let expr_buf: String = chars[start..end].iter().collect();

                let mut p = Parser::new_expr_only(expr_buf, span);
                let expr = p.parse_expr()?;

                parts.push(UiTextPart::Interpolation(expr, span));
            } else {
                buf.push(chars[i]);
                i += 1;
            }
        }

        if !buf.is_empty() {
            parts.push(UiTextPart::Literal(buf, span));
        }

        Ok(UiText { parts, span })
    }
}
