//! Text helpers for building a code editor in Tint itself: syntax
//! highlighting on the real lexer, and line metrics for sizing.

use super::*;
use tint_lexer::{collect_tokens, Lexer, TokenKind};

/// Highlight class of a token: plain, keyword, atom, string, number, type,
/// fn, punct, op, opkw, comment.
fn class_of(kind: TokenKind, text: &str, next_is_paren: bool) -> &'static str {
    use TokenKind::*;
    if text.starts_with('#') {
        return "atom";
    }
    match kind {
        String => "string",
        Number => "number",
        True | False | Underscore | SelfKw => "atom",
        Ident => {
            if next_is_paren {
                "fn"
            } else if text.chars().next().is_some_and(|c| c.is_uppercase()) {
                "type"
            } else {
                "plain"
            }
        }
        PathSep | Pipe | OrOr => "opkw",
        Plus | Minus | Star | Slash | Percent | Eq | PlusEq | MinusEq | StarEq | SlashEq | EqEq
        | NotEq | Less | Greater | LessEq | GreaterEq | AndAnd | Ampersand | Bang | Question
        | FatArrow | Arrow | DotDot | DotDotDot => "op",
        LBrace | RBrace | LParen | RParen | LAngle | RAngle | LBracket | RBracket | Comma
        | Colon | Semicolon | Dot => "punct",
        Eof => "plain",
        _ => "keyword",
    }
}

fn pair(text: &str, class: &str) -> EvalValue {
    EvalValue::List(vec![
        EvalValue::String(text.to_string()),
        EvalValue::String(class.to_string()),
    ])
}

/// Splits text the lexer skipped (whitespace, `//` comments) into segments.
fn push_gap(out: &mut Vec<EvalValue>, gap: &str) {
    let mut rest = gap;
    while !rest.is_empty() {
        match rest.find("//") {
            Some(0) => {
                let end = rest.find('\n').unwrap_or(rest.len());
                out.push(pair(&rest[..end], "comment"));
                rest = &rest[end..];
            }
            Some(at) => {
                out.push(pair(&rest[..at], "plain"));
                rest = &rest[at..];
            }
            None => {
                out.push(pair(rest, "plain"));
                break;
            }
        }
    }
}

/// `[[text, class], ...]` covering the whole source, in order.
pub(super) fn highlight(src: &str) -> EvalValue {
    let tokens = collect_tokens(&mut Lexer::new(src));
    let mut out = Vec::new();
    let mut at = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == TokenKind::Eof {
            break;
        }
        let (start, end) = (token.span.start.offset, token.span.end.offset);
        if start < at
            || end > src.len()
            || !src.is_char_boundary(start)
            || !src.is_char_boundary(end)
        {
            continue;
        }
        if start > at {
            push_gap(&mut out, &src[at..start]);
        }
        let text = &src[start..end];
        let next_is_paren = tokens
            .get(index + 1)
            .is_some_and(|next| next.kind == TokenKind::LParen);
        out.push(pair(
            text,
            class_of(token.kind.clone(), text, next_is_paren),
        ));
        at = end;
    }
    if at < src.len() {
        push_gap(&mut out, &src[at..]);
    }
    EvalValue::List(out)
}

pub(super) fn line_count(src: &str) -> f64 {
    src.split('\n').count() as f64
}

pub(super) fn max_line_len(src: &str) -> f64 {
    src.split('\n')
        .map(|line| line.chars().count())
        .max()
        .unwrap_or(0) as f64
}

pub(super) fn line_numbers(src: &str) -> String {
    (1..=src.split('\n').count())
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(value: EvalValue) -> Vec<(String, String)> {
        let EvalValue::List(items) = value else {
            panic!("list")
        };
        items
            .into_iter()
            .map(|item| {
                let EvalValue::List(pair) = item else {
                    panic!("pair")
                };
                let (EvalValue::String(a), EvalValue::String(b)) = (&pair[0], &pair[1]) else {
                    panic!("strings")
                };
                (a.clone(), b.clone())
            })
            .collect()
    }

    #[test]
    fn highlight_covers_source_exactly() {
        let src = "fn a() { // hi\n  1 + \"s\" #fff }\n";
        let segs = flat(highlight(src));
        let joined: String = segs.iter().map(|(t, _)| t.as_str()).collect();
        assert_eq!(joined, src);
        let has = |text: &str, class: &str| segs.iter().any(|(t, c)| t == text && c == class);
        assert!(has("fn", "keyword") && has("a", "fn") && has("// hi", "comment"));
        assert!(
            has("1", "number") && has("\"s\"", "string") && has("#fff", "atom") && has("+", "op")
        );
    }

    #[test]
    fn highlight_survives_half_typed_input() {
        let src = "Card { text::\"unterminated";
        let joined: String = flat(highlight(src))
            .iter()
            .map(|(t, _)| t.clone())
            .collect();
        assert_eq!(joined, src);
    }

    #[test]
    fn line_metrics() {
        assert_eq!(line_count("a\nbb\n"), 3.0);
        assert_eq!(max_line_len("a\nbbb"), 3.0);
        assert_eq!(line_numbers("x\ny"), "1\n2");
    }
}
