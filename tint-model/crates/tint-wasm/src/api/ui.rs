#[derive(Serialize, Default)]
pub struct UiRunResult {
    pub ok: bool,
    pub item_count: usize,
    pub tree: Vec<tint_runtime::ui::render::UiRenderNode>,
    pub error: Option<String>,
    pub error_span: Option<SpanInfo>,
}

/// Parses `source`, runs it, and calls `ui_fn_name` (a `ui fn`, not a
/// plain `fn`) with NO arguments -- same no-arg-only limitation as
/// `run()`/`tint-cli`'s `run` command, for the same reason (no CLI/JS-side
/// argument marshalling yet). Returns the evaluated render tree: tags
/// with resolved text, if{}/for{} directives actually applied, and a
/// best-effort CSS `style` list translated from Tint's modifiers (see
/// tint_runtime::ui::render's module doc comment for exactly what's
/// covered). The sandbox turns this into real DOM elements.
#[wasm_bindgen]
pub fn render_ui(source: &str, ui_fn_name: &str) -> JsValue {
    let mut result = UiRunResult::default();

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

    match vm.render_ui_fn(ui_fn_name, &[]) {
        Ok(tree) => {
            result.ok = true;
            result.tree = tree;
        }
        Err(e) => {
            result.error = Some(format!("{:?}", e));
        }
    }
    serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
}

#[wasm_bindgen]
pub fn tint_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}
