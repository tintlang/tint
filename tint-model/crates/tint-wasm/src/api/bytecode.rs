/// Compiles Tint source into the versioned program blob consumed by
/// `DomSession::from_bytecode` and `reload_bytecode`.
///
/// This is primarily useful to hosts that want to cache compiled programs.
/// Applications may use source mode directly; no bytecode step is required
/// for development.
#[wasm_bindgen]
pub fn compile_bytecode(source: &str) -> Result<Vec<u8>, JsValue> {
    let tokens = collect_tokens(&mut Lexer::new(source));
    let mut parser = Parser::new(tokens);
    let program = parser
        .parse_program()
        .map_err(|error| JsValue::from_str(&format!("{:?}", error)))?;

    let semantic_errors = tint_semantics::SemanticChecker::new(
        tint_semantics::prelude::CheckerContext::default(),
    )
    .check(&program);
    if !semantic_errors.is_empty() {
        return Err(JsValue::from_str(&format!("semantic errors: {:?}", semantic_errors)));
    }

    tint_runtime::bytecode::encode(&program)
        .map_err(|error| JsValue::from_str(&error))
}
