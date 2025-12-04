use crate::{Parser, error::*};
use rune_ast::{Expr, Pattern, Span};
use rune_lexer::TokenKind;

impl Parser {
    pub(crate) fn parse_match_expression(&mut self) -> PResult<Expr> {
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
        let scrutinee = match self.parse_expr_until(TokenKind::LBrace) {
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

            self.stream.expect(TokenKind::FatArrow)?;
            eprintln!("MATCH: got '=>'");

            // VALUE expr
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

            if self.stream.consume_if(TokenKind::Comma) {
                eprintln!("MATCH: consumed trailing comma");
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
}
