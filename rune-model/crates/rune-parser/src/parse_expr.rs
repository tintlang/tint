// rune-parser/parse_expr.rs

use rune_lexer::TokenKind;
use rune_ast::{Expr, Span};
use crate::{Parser, error::*};
use rune_ast::Pattern;


impl Parser {
    // ENTRY POINT
    pub fn parse_expr(&mut self) -> PResult<Expr> {
        eprintln!(
            "[EXPR] parse_expr start: token={:?} '{}' at {:?}, in_pattern={}",
            self.stream.peek_kind(),
            self.stream.peek().lexeme,
            self.stream.peek().span,
            self.in_pattern
        );

        if self.stream.peek_kind() == TokenKind::Match {
            return self.parse_match_expression();
        }

        let out = self.parse_binary_expr(0)?;
        eprintln!(
            "[EXPR] parse_expr end:   token={:?} '{}' at {:?}, in_pattern={}",
            self.stream.peek_kind(),
            self.stream.peek().lexeme,
            self.stream.peek().span,
            self.in_pattern
        );
        Ok(out)
    }


    fn parse_array_literal(&mut self) -> PResult<Expr> {
        let start = self.stream.next().span; // consume '['

        // empty array: []
        if self.stream.consume_if(TokenKind::RBracket) {
            let end = self.stream.last_span();
            return Ok(Expr::Array {
                items: vec![],
                span: Span::merge(start, end),
            });
        }

        let mut items = Vec::new();

        loop {
            let item = self.parse_expr()?;
            items.push(item);

            // no comma → break
            if !self.stream.consume_if(TokenKind::Comma) {
                break;
            }

            // allow trailing comma: [1,2,]
            if self.stream.peek_kind() == TokenKind::RBracket {
                break;
            }
        }

        self.stream.expect(TokenKind::RBracket)?;
        let end = self.stream.last_span();

        Ok(Expr::Array {
            items,
            span: Span::merge(start, end),
        })
    }

fn parse_interpolated_string(&mut self, raw: String, span: Span) -> PResult<Expr> {
    use rune_ast::StringPart::{Text, Expr as PartExpr};

    let mut parts = Vec::new();
    let chars: Vec<char> = raw.chars().collect();

    let mut i = 0;
    let mut buf = String::new();

    while i < chars.len() {
        match chars[i] {
            '{' => {
                // push accumulated text
                if !buf.is_empty() {
                    parts.push(Text(buf.clone()));
                    buf.clear();
                }

                // find closing '}'
                i += 1;
                let start = i;
                let mut depth = 1;

                while i < chars.len() && depth > 0 {
                    if chars[i] == '{' { depth += 1; }
                    if chars[i] == '}' { depth -= 1; }
                    i += 1;
                }

                let expr_src = &raw[start..i-1];

                // parse expression inside {}
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

fn parse_match_expression(&mut self) -> PResult<Expr> {
    let start_tok = self.stream.peek();
    eprintln!("\n==================== MATCH ====================");
    eprintln!("MATCH: start at {:?}", start_tok.span);

    // match
    let start = self.stream.next().span;
    eprintln!("MATCH: consumed 'match', next token={:?} '{}' @ {:?}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    // scrutinee
    eprintln!("MATCH: --- parsing scrutinee ---");
    let scrutinee = match self.parse_expr() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ MATCH: scrutinee parse FAILED: {:?}", e);
            return Err(e);
        }
    };

    eprintln!("MATCH: scrutinee OK = {:?}, next token={:?} '{}' @ {:?}",
        scrutinee,
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    // expect {
    eprintln!("MATCH: expect LBrace '{{'}} ...");
    let brace = self.stream.expect(TokenKind::LBrace)?;
    eprintln!("MATCH: got '{{}}' @ {:?}", brace.span);

    eprintln!("MATCH: entering arms, first token={:?} '{}' @ {:?}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    let mut arms = Vec::new();

    // ARMS LOOP
    loop {
        // End?
        if self.stream.consume_if(TokenKind::RBrace) {
            eprintln!("MATCH: reached closing '}}' @ {:?}", self.stream.last_span());
            break;
        }

        eprintln!("\nMATCH: ----- NEW ARM -----");
        eprintln!("MATCH: arm start token={:?} '{}' @ {:?}",
            self.stream.peek_kind(),
            self.stream.peek().lexeme,
            self.stream.peek().span
        );

        // PATTERN
        eprintln!("MATCH: --- parsing pattern ---");

        let old = self.in_pattern;
        self.in_pattern = true;

        let pat_res = self.parse_pattern();
        self.in_pattern = old;

        match pat_res {
            Ok(ref p) => eprintln!("MATCH: pattern OK = {:?}", p),
            Err(e) => {
                eprintln!("❌ MATCH: pattern FAILED: {:?}", e);
                return Err(e);
            }
        }

        let pat = pat_res?;

        // expect =>
        eprintln!(
            "MATCH: expect FAT ARROW '=>' , current={:?} '{}' @ {:?}",
            self.stream.peek_kind(),
            self.stream.peek().lexeme,
            self.stream.peek().span
        );

        if let Err(e) = self.stream.expect(TokenKind::FatArrow) {
            eprintln!("❌ MATCH: expected =>, got error {:?}", e);
            return Err(e);
        }

        eprintln!("MATCH: got '=>'");

        // VALUE expression
        eprintln!("MATCH: --- parsing value expression ---");
        let value_res = self.parse_expr();

        match value_res {
            Ok(ref v) => eprintln!("MATCH: value OK = {:?}, next token={:?} '{}' @ {:?}",
                v,
                self.stream.peek_kind(),
                self.stream.peek().lexeme,
                self.stream.peek().span
            ),
            Err(e) => {
                eprintln!("❌ MATCH: value expr FAILED: {:?}", e);
                return Err(e);
            }
        }

        let value = value_res?;

        arms.push((pat, value));

        // optional comma
        if self.stream.consume_if(TokenKind::Comma) {
            eprintln!("MATCH: consumed trailing comma, next token={:?} '{}' @ {:?}",
                self.stream.peek_kind(),
                self.stream.peek().lexeme,
                self.stream.peek().span
            );
        } else {
            eprintln!("MATCH: no comma, next token={:?} '{}' @ {:?}",
                self.stream.peek_kind(),
                self.stream.peek().lexeme,
                self.stream.peek().span
            );
        }
    }

    eprintln!("================ END MATCH ================\n");

    let end = self.stream.last_span();

    Ok(Expr::Match {
        scrutinee: Box::new(scrutinee),
        arms,
        span: Span::merge(start, end),
    })
}

pub fn parse_let_pattern(&mut self) -> PResult<Pattern> {
    match self.stream.peek_kind() {

        TokenKind::Ident => {
            // SPAN копируем до parse_ident()
            let span = self.stream.peek().span;
            let id = self.parse_ident()?;
            Ok(Pattern::Ident(id, span))
        }

        TokenKind::LParen => {
            // tuple pattern
            self.parse_tuple_pattern()
        }

        _ => Err(ParserError::Message {
            msg: "Invalid pattern in let (only `name` or `(x,y)` allowed)".into(),
            span: self.stream.peek().span,
        })
    }
}

    // PRIMARY EXPRESSIONS
    // number, string, ident, paren, lambda
    fn parse_primary(&mut self) -> PResult<Expr> {
           eprintln!(
        "[PRIMARY] token={:?} '{}' at {:?}, in_pattern={}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span,
        self.in_pattern
    );
        let tok = self.stream.peek().clone();

        match tok.kind {
            TokenKind::Number => {
                let t = self.stream.next();
                Ok(Expr::Number(t.lexeme, t.span))
            }

            TokenKind::String => {
                let t = self.stream.next();

                // если нет интерполяции — обычная строка
                if !t.lexeme.contains('{') {
                    return Ok(Expr::String(t.lexeme, t.span));
                }

                // иначе — разбор интерполированной строки
                return self.parse_interpolated_string(t.lexeme, t.span);
            }

             TokenKind::True => {
                let t = self.stream.next();
                return Ok(Expr::Bool(true, t.span));
            }

            TokenKind::False => {
                let t = self.stream.next();
                return Ok(Expr::Bool(false, t.span));
            }

            TokenKind::Ident => {
                let t = self.stream.next(); // name or struct name

                // STRUCT INIT or STRUCT UPDATE: name { ... }
                // We allow both UpperCase (types) and lowerCase (update syntax)
                if self.stream.peek_kind() == TokenKind::LBrace {
                    return self.parse_struct_init(t.lexeme, t.span);
                }

                // Otherwise normal identifier
                return Ok(Expr::Ident(t.lexeme, t.span));
            }

            TokenKind::Match => {
                return self.parse_match_expression();
            }

            // λ LAMBDA: |x| expr
            TokenKind::PipeLambda => {
                let start = self.stream.next().span;

                // parse params: |x,y|
                let mut params = Vec::new();
                loop {
                    let name = self.parse_ident()?;
                    params.push(name);

                    if !self.stream.consume_if(TokenKind::Comma) {
                        break;
                    }
                }

                // expect closing |
                self.stream.expect(TokenKind::PipeLambda)?;
                let body = self.parse_expr()?;
                let span = Span::merge(start, body.span());

                Ok(Expr::Lambda { params, body: Box::new(body), span })
            }

            // ARRAY LITERAL: [expr, expr, ...]
            TokenKind::LBracket => {
                return self.parse_array_literal();
            }

            // Parentheses
             TokenKind::LParen => {
                let open = self.stream.next().span;

                // UNIT: ()
                if self.stream.consume_if(TokenKind::RParen) {
                    return Ok(Expr::Unit(open));
                }

                // parse first element
                let first = self.parse_expr()?;

                // tuple?  (expr, ...)
                if self.stream.consume_if(TokenKind::Comma) {

                    // Tuple literal:
                    let mut items = vec![first];

                    // parse remaining items
                    loop {
                        let elem = self.parse_expr()?;
                        items.push(elem);

                        if !self.stream.consume_if(TokenKind::Comma) {
                            break;
                        }
                    }

                    self.stream.expect(TokenKind::RParen)?;
                    let end = self.stream.last_span();
                    return Ok(Expr::Tuple {
                        items,
                        span: Span::merge(open, end),
                    });
                }

                // NOT a tuple → just grouping
                self.stream.expect(TokenKind::RParen)?;
                Ok(Expr::Paren(Box::new(first), open))
            }

            // Unary ops: !expr, -expr
            TokenKind::Bang | TokenKind::Minus => {
                let op = self.stream.next();
                let right = self.parse_primary()?;
                let span = Span::merge(op.span, right.span());

                Ok(Expr::Unary {
                    op: op.lexeme,
                    expr: Box::new(right),
                    span,
                })
            }

            _ => Err(ParserError::Message {
                msg: format!("Unexpected token in expression: {:?}", tok.kind),
                span: tok.span,
            }),
        }
    }

    fn parse_fn_call_arg(&mut self) -> PResult<Expr> {
        // named arg?   name { expr }
        if self.stream.peek_kind() == TokenKind::Ident
            && self.stream.peek2_kind() == TokenKind::LBrace
        {
            // name
            let name = self.parse_ident()?;
            let start = self.stream.last_span();

            // {
            self.stream.expect(TokenKind::LBrace)?;
            let value = self.parse_expr()?;
            self.stream.expect(TokenKind::RBrace)?;
            let end = self.stream.last_span();

            return Ok(Expr::NamedArg {
                name,
                value: Box::new(value),
                span: Span::merge(start, end),
            });
        }

        // otherwise normal expression
        self.parse_expr()
    }

        // CALLS, FIELD ACCESS, NAMESPACE ACCESS, INDEXING
        // After parsing primary: foo(), a.b, a::b, a[b]
fn parse_postfix(&mut self) -> PResult<Expr> {
    if self.in_pattern {
        eprintln!(
            "[POSTFIX] ERROR: postfix inside pattern at {:?}, token={:?}",
            self.stream.peek().span,
            self.stream.peek_kind()
        );
        return Err(ParserError::Message {
            msg: "postfix not allowed in pattern mode".into(),
            span: self.stream.peek().span,
        });
    }

    eprintln!(
        "[POSTFIX] primary start: token={:?} '{}' at {:?}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    let mut expr = self.parse_primary()?;

    loop {
        let tok = self.stream.peek().clone();
        eprintln!(
            "[POSTFIX] loop: token={:?} '{}' at {:?}",
            tok.kind,
            tok.lexeme,
            tok.span
        );

        match tok.kind {
            // -------- FUNCTION CALL --------
            TokenKind::LParen => {
                let start_span = expr.span();
                self.stream.next(); // '('
                eprintln!("[POSTFIX] start call");

                let mut args = Vec::new();

                if !self.stream.consume_if(TokenKind::RParen) {
                    loop {
                        let arg = self.parse_fn_call_arg()?;
                        args.push(arg);

                        if !self.stream.consume_if(TokenKind::Comma) {
                            break;
                        }
                    }
                    self.stream.expect(TokenKind::RParen)?;
                }

                let end_span = self.stream.last_span();
                expr = Expr::Call {
                    target: Box::new(expr),
                    args,
                    span: Span::merge(start_span, end_span),
                };

                eprintln!("[POSTFIX] parsed call OK");
            }

            // -------- FIELD ACCESS --------
            TokenKind::Dot => {
                let dot = self.stream.next(); // '.'
                eprintln!("[POSTFIX] start field");

                let field = self.parse_ident()?;
                let span = Span::merge(expr.span(), dot.span);

                expr = Expr::Field {
                    target: Box::new(expr),
                    field: field.clone(),
                    span,
                };

                eprintln!("[POSTFIX] parsed field '{}'", field);
            }

            // -------- NAMESPACE ACCESS --------
            TokenKind::PathSep => {
                let sep = self.stream.next(); // '::'
                eprintln!("[POSTFIX] start namespace");

                let item = self.parse_ident()?;
                let span = Span::merge(expr.span(), sep.span);

                expr = Expr::Namespace {
                    base: Box::new(expr),
                    item: item.clone(),
                    span,
                };

                eprintln!("[POSTFIX] parsed namespace '{}'", item);
            }

            // -------- INDEXING --------
            TokenKind::LBracket => {
                let start = self.stream.next().span; // '['
                eprintln!("[POSTFIX] start index");

                let index_expr = self.parse_expr()?;
                self.stream.expect(TokenKind::RBracket)?;

                let span = Span::merge(expr.span(), index_expr.span());
                expr = Expr::Index {
                    target: Box::new(expr),
                    index: Box::new(index_expr),
                    span,
                };

                eprintln!("[POSTFIX] parsed index");
            }

            _ => break,
        }
    }

    eprintln!(
        "[POSTFIX] exit: next token={:?} '{}' at {:?}",
        self.stream.peek_kind(),
        self.stream.peek().lexeme,
        self.stream.peek().span
    );

    Ok(expr)
}



    // Pratt parser for binary operators
    fn parse_binary_expr(&mut self, min_prec: u8) -> PResult<Expr> {
        let mut left = self.parse_postfix()?;

        loop {
            let op_tok = self.stream.peek().clone();
            let prec = op_tok.kind.binary_precedence();

            if prec < min_prec || prec == 0 {
                break;
            }

            let op = self.stream.next(); // consume operator
            let right = self.parse_binary_expr(prec + 1)?;
            let span = Span::merge(left.span(), right.span());

            left = Expr::Binary {
                left: Box::new(left),
                op: op.lexeme,
                right: Box::new(right),
                span,
            };
        }

        Ok(left)
    }
}
