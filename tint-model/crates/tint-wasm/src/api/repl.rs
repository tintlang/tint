/// One entry's worth of `TintRepl::eval()` output.
#[derive(Serialize, Default)]
pub struct ReplLineResult {
    /// True if `line` was a definition (fn/struct/enum/...) that got
    /// committed to the session -- no `value`, just persisted for later
    /// entries to use.
    pub defined: bool,
    pub value: Option<String>,
    pub output: String,
    pub error: Option<String>,
}

/// A stateful REPL session for the sandbox's terminal panel, unlike
/// `check()`/`run()`/`render_ui()` above which each construct a fresh,
/// throwaway `TintVM` (see this crate's other functions' doc comments and
/// sandbox/README.md's "Known limitations"). Backed by
/// `tint_runtime::repl::ReplSession` -- see that module's doc comment for
/// exactly how persistence works (re-parse-and-rerun-everything, not true
/// incrementality) and why every call is panic-hardened (an interactive
/// REPL hits the IR VM's panic-on-runtime-error paths, e.g. calling an
/// undefined function, as a normal/frequent case, not a rare one).
#[wasm_bindgen]
pub struct TintRepl {
    session: tint_runtime::repl::ReplSession,
}

#[wasm_bindgen]
impl TintRepl {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TintRepl {
        TintRepl {
            session: tint_runtime::repl::ReplSession::new(),
        }
    }

    /// Evaluate one line/entry against everything defined earlier in this
    /// session.
    pub fn eval(&mut self, line: &str) -> JsValue {
        let outcome = self.session.eval(line);
        let result = ReplLineResult {
            defined: outcome.defined,
            value: outcome.value,
            output: outcome.output,
            error: outcome.error,
        };
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Forget every definition made in this session so far.
    pub fn reset(&mut self) {
        self.session.reset();
    }
}

impl Default for TintRepl {
    fn default() -> Self {
        Self::new()
    }
}
