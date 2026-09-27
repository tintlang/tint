#[derive(Serialize, Default)]
pub struct UiSessionResult {
    pub ok: bool,
    pub tree: Vec<tint_runtime::ui::render::UiRenderNode>,
    pub error: Option<String>,
}

/// A stateful UI session for the sandbox's preview pane, backing real
/// `click||`/`hover_in||`/`hover_out||` interactivity -- unlike
/// `render_ui()` above, which builds a fresh, throwaway `TintVM` on every
/// call (fine for "re-render on every keystroke", useless for "remember
/// that the popover is open"), this wraps a single
/// `tint_runtime::ui_session::UiSession` that stays alive for the
/// session's lifetime. See that module's doc comment for exactly how
/// `state` persistence and handler dispatch work under here.
///
/// Construction can fail (bad source, unknown `ui_fn_name`) without a JS
/// exception -- the failure is stored and reported from `tree()`/
/// `dispatch()` instead, so `new UiSession(...)` itself never throws.
#[wasm_bindgen]
pub struct UiSession {
    inner: Option<tint_runtime::ui_session::UiSession>,
    init_error: Option<String>,
}

#[wasm_bindgen]
impl UiSession {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, ui_fn_name: &str) -> UiSession {
        match tint_runtime::ui_session::UiSession::new(source, ui_fn_name) {
            Ok(session) => UiSession {
                inner: Some(session),
                init_error: None,
            },
            Err(e) => UiSession {
                inner: None,
                init_error: Some(e),
            },
        }
    }

    /// The session's current tree (a fresh build, not cached).
    pub fn tree(&mut self) -> JsValue {
        let mut result = UiSessionResult::default();
        match &mut self.inner {
            None => result.error = self.init_error.clone(),
            Some(session) => match session.render() {
                Ok(tree) => {
                    result.ok = true;
                    result.tree = tree;
                }
                Err(e) => result.error = Some(e),
            },
        }
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Points this SAME `UiSession` at different source -- re-parsing
    /// `source` as `ui_fn_name` into a brand-new inner session (this
    /// resets `state`, exactly like constructing a new session would)
    /// and returning the freshly-rendered tree in the same shape as
    /// `tree()`/`dispatch()`. Mirrors `DomSession::reload` for this
    /// JS-tree-consuming path: the sandbox's live editor calls this on
    /// every debounced edit instead of throwing away and reconstructing
    /// the whole `UiSession` (and losing the ability to report a
    /// bad-source error the same way `tree()` does) per keystroke.
    pub fn reload(&mut self, source: &str, ui_fn_name: &str) -> JsValue {
        match tint_runtime::ui_session::UiSession::new(source, ui_fn_name) {
            Ok(session) => {
                self.inner = Some(session);
                self.init_error = None;
            }
            Err(e) => {
                self.inner = None;
                self.init_error = Some(e);
            }
        }
        self.tree()
    }

    /// Runs `handler` (a plain `fn`, no args) against this session's
    /// persistent state, then returns the rebuilt tree -- see
    /// `tint_runtime::ui_session`'s doc comment for why this needs a
    /// real, persistent session rather than a fresh `render_ui()` call.
    pub fn dispatch(&mut self, handler: &str) -> JsValue {
        let mut result = UiSessionResult::default();
        match &mut self.inner {
            None => result.error = self.init_error.clone(),
            Some(session) => match session.dispatch(handler) {
                Ok(tree) => {
                    result.ok = true;
                    result.tree = tree;
                }
                Err(e) => result.error = Some(e),
            },
        }
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Dispatches a Tint frame handler with elapsed seconds as `dt`.
    pub fn dispatch_frame(&mut self, handler: &str, dt: f64) -> JsValue {
        let mut result = UiSessionResult::default();
        match &mut self.inner {
            None => result.error = self.init_error.clone(),
            Some(session) => match session.dispatch_with_args(
                handler,
                &[tint_evaluator::Value::Number(dt)],
            ) {
                Ok(tree) => {
                    result.ok = true;
                    result.tree = tree;
                }
                Err(e) => result.error = Some(e),
            },
        }
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Returns and clears requests queued by Tint's `http_get(url)` calls.
    /// The host should perform fetch and later call `dispatch_http`.
    pub fn take_http_requests(&mut self) -> JsValue {
        let requests = match &mut self.inner {
            Some(session) => session
                .take_http_requests()
                .into_iter()
                .map(|request| HttpRequestResult {
                    id: request.id,
                    method: request.method,
                    url: request.url,
                })
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        serde_wasm_bindgen::to_value(&requests).unwrap_or(JsValue::NULL)
    }

    /// Delivers an HTTP completion to a Tint handler as
    /// `(request_id, status, body)`.
    pub fn dispatch_http(&mut self, handler: &str, request_id: f64, status: f64, body: &str) -> JsValue {
        let mut result = UiSessionResult::default();
        match &mut self.inner {
            None => result.error = self.init_error.clone(),
            Some(session) => match session.dispatch_with_args(
                handler,
                &[
                    tint_evaluator::Value::Number(request_id),
                    tint_evaluator::Value::Number(status),
                    tint_evaluator::Value::String(body.to_string()),
                ],
            ) {
                Ok(tree) => {
                    result.ok = true;
                    result.tree = tree;
                }
                Err(e) => result.error = Some(e),
            },
        }
        serde_wasm_bindgen::to_value(&result).unwrap_or(JsValue::NULL)
    }

    /// Reads the VM storage as a plain JS object for localStorage syncing.
    pub fn storage_snapshot(&self) -> JsValue {
        let values = self
            .inner
            .as_ref()
            .map(|session| session.storage_snapshot())
            .unwrap_or_default();
        serde_wasm_bindgen::to_value(&values).unwrap_or(JsValue::NULL)
    }

    /// Merges a plain JS object into the VM storage.
    pub fn hydrate_storage(&mut self, values: JsValue) -> Result<(), JsValue> {
        let values: HashMap<String, String> = serde_wasm_bindgen::from_value(values)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        match &mut self.inner {
            Some(session) => {
                session.hydrate_storage(values);
                Ok(())
            }
            None => Err(JsValue::from_str(
                self.init_error.as_deref().unwrap_or("session is unavailable"),
            )),
        }
    }
}
use std::collections::HashMap;

#[derive(Serialize)]
struct HttpRequestResult {
    id: u64,
    method: String,
    url: String,
}
