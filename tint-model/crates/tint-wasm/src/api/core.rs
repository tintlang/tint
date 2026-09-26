#[derive(Serialize)]
pub struct SpanInfo {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

#[derive(Serialize, Default)]
pub struct RunResult {
    pub ok: bool,
    pub item_count: usize,
    pub value: Option<String>,
    pub output: String,
    pub error: Option<String>,
    pub error_span: Option<SpanInfo>,
}

fn parse_source(code: &str) -> Result<Program, ParserError> {
    let tokens = collect_tokens(&mut Lexer::new(code));
    let mut parser = Parser::new(tokens);
    parser.parse_program()
}

fn span_of(e: &ParserError) -> Span {
    match e {
        ParserError::Message { span, .. } => *span,
        ParserError::Unexpected { span, .. } => *span,
    }
}

fn to_span_info(span: Span) -> SpanInfo {
    SpanInfo {
        start_line: span.start.line,
        start_col: span.start.column,
        end_line: span.end.line,
        end_col: span.end.column,
    }
}

fn has_fn(program: &Program, name: &str) -> bool {
    program
        .items
        .iter()
        .any(|item| matches!(item, Item::Fn(f) if f.name == name))
}

/// Lex + parse `source` only. Mirrors `tint check`.
#[wasm_bindgen]
pub fn check(source: &str) -> JsValue {
    let mut result = RunResult::default();
    match parse_source(source) {
        Ok(program) => {
            result.ok = true;
            result.item_count = program.items.len();
        }
        Err(e) => {
            result.error_span = Some(to_span_info(span_of(&e)));
            result.error = Some(format!("{:?}", e));
        }
    }
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

/// Parse, compile to IR, and call `entry` with no arguments. Mirrors
/// `tint run <file> [entry]`. Any output from print()/dbg() inside the
/// program is captured and returned in `output` rather than lost, since
/// there is no real stdout to write to here.
#[wasm_bindgen]
pub fn run(source: &str, entry: &str) -> JsValue {
    let mut result = RunResult::default();

    let program = match parse_source(source) {
        Ok(p) => p,
        Err(e) => {
            result.error_span = Some(to_span_info(span_of(&e)));
            result.error = Some(format!("{:?}", e));
            return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
        }
    };
    result.item_count = program.items.len();

    let mut vm = TintVM::new();
    vm.run_program(&program);

    if !has_fn(&program, entry) {
        // Compiled fine, nothing to call -- not an error (e.g. a file
        // that only declares structs/enums, or a typo'd entry name the
        // sandbox should surface as "nothing ran", not a crash).
        result.ok = true;
        result.output = tint_evaluator::output::take_output();
        return serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL);
    }

    match vm.call_fn(entry, &[], Span::dummy()) {
        Ok(v) => {
            result.ok = true;
            result.value = Some(format!("{}", v));
        }
        Err(e) => {
            result.error = Some(format!("{:?}", e));
        }
    }
    result.output = tint_evaluator::output::take_output();
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}
