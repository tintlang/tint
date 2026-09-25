use crate::{Parser};
use crate::error::*;
use tint_ast::*;
use tint_lexer::TokenKind;

impl Parser {

    /// Parse:
    /// kernel name(param: Type, ...) -> Type? { <block> }
pub fn parse_kernel(&mut self) -> PResult<KernelDecl> {
    let start = self.stream.expect(TokenKind::Kernel)?.span;

    let name = self.parse_ident()?;

    self.stream.expect(TokenKind::LParen)?;
    let params = self.parse_params()?;
    self.stream.expect(TokenKind::RParen)?;

    let ret_ty = if self.stream.consume_if(TokenKind::Arrow) {
        Some(self.parse_type()?)
    } else {
        None
    };

    let body = self.parse_block()?;

    let span = Span::merge(start, body.span);

    Ok(KernelDecl {
        name,
        params,
        ret_ty,
        body,
        span,
    })
}

}
