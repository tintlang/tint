#[wasm_bindgen]
impl DomSession {
    #[cfg(feature = "interpreter")]
    #[wasm_bindgen(constructor)]
    pub fn new(
        source: &str,
        ui_fn_name: &str,
        container_id: &str,
        natives: Option<js_sys::Object>,
    ) -> DomSession {
        dom_tree_shape();
        let natives = natives_from_js(natives);
        Self::from_inner(
            InnerSession::new_with_natives(
                source,
                ui_fn_name,
                // Saved values only matter to a program that reads them.
                if source.contains("storage_") { load_browser_storage() } else { Default::default() },
                &natives,
            )
            .map(boxed),
            container_id,
            false,
            natives,
        )
    }

    /// Creates a session from a versioned Tint bytecode blob. The browser
    /// host can choose source mode for development and this mode for release
    /// artifacts without changing the DOM/session API.
    #[cfg(feature = "interpreter")]
    #[cfg(feature = "bytecode")]
    pub fn from_bytecode(
        bytes: &[u8],
        ui_fn_name: &str,
        container_id: &str,
        natives: Option<js_sys::Object>,
    ) -> DomSession {
        dom_tree_shape();
        let natives = natives_from_js(natives);
        Self::from_inner(
            InnerSession::from_bytecode_with_natives(bytes, ui_fn_name, &natives).map(boxed),
            container_id,
            false,
            natives,
        )
    }

    /// Re-renders the session's current tree into the container,
    /// replacing whatever was there. Returns `Some(error)` instead of
    /// throwing; `None` means it went fine.
    pub fn rerender(&mut self) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        sync_viewport_width(&shared);
        let tree = match shared.session.borrow_mut().render() {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }

    /// Runs `handler` against this session's persistent state (same
    /// dispatch `tint_runtime::ui_session::UiSession::dispatch` already
    /// does for the Svelte path), then rebuilds the DOM from the result.
    pub fn dispatch(&mut self, handler: &str) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        sync_viewport_width(&shared);
        let tree = match shared.session.borrow_mut().dispatch(handler) {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }

    /// Dispatches a `frame||handler` with elapsed seconds as `dt`.
    pub fn dispatch_frame(&mut self, handler: &str, dt: f64) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        sync_viewport_width(&shared);
        let tree = match shared.session.borrow_mut().dispatch_with_args(
            handler,
            &[tint_evaluator::Value::Number(dt)],
        ) {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }

    /// Returns and clears queued `http_get(url)` work as a JS array. The
    /// browser host performs fetch and feeds completion to `dispatch_http`.
    pub fn take_http_requests(&mut self) -> JsValue {
        let requests = match &self.shared {
            Some(shared) => shared
                .session
                .borrow_mut()
                .take_http_requests()
                .into_iter()
                .map(|request| HttpRequestJson {
                    id: request.id,
                    method: request.method,
                    url: request.url,
                })
                .collect::<Vec<_>>(),
            None => Vec::new(),
        };
        serde_wasm_bindgen::to_value(&requests).unwrap_or(JsValue::NULL)
    }

    /// Delivers `(request_id, status, body)` to a Tint HTTP handler and
    /// updates the DOM with its result.
    pub fn dispatch_http(
        &mut self,
        handler: &str,
        request_id: f64,
        status: f64,
        body: &str,
    ) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        let tree = match shared.session.borrow_mut().dispatch_with_args(
            handler,
            &[
                tint_evaluator::Value::Number(request_id),
                tint_evaluator::Value::Number(status),
                tint_evaluator::Value::String(body.to_string()),
            ],
        ) {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }

    pub fn storage_snapshot(&self) -> JsValue {
        let values = self
            .shared
            .as_ref()
            .map(|shared| shared.session.borrow().storage_snapshot())
            .unwrap_or_default();
        serde_wasm_bindgen::to_value(&values).unwrap_or(JsValue::NULL)
    }

    pub fn hydrate_storage(&mut self, values: JsValue) -> Result<(), JsValue> {
        let values: std::collections::HashMap<String, String> =
            serde_wasm_bindgen::from_value(values)
                .map_err(|error| JsValue::from_str(&error.to_string()))?;
        match &self.shared {
            Some(shared) => {
                shared.session.borrow_mut().hydrate_storage(values);
                Ok(())
            }
            None => Err(JsValue::from_str(
                self.init_error.as_deref().unwrap_or("session is unavailable"),
            )),
        }
    }

    /// Points this SAME `DomSession` at different source -- re-parsing
    /// `source` as `ui_fn_name` into a brand-new inner session and
    /// rendering it, in place of the one built at construction time.
    /// `dispatch` above replays a click/hover against the program that's
    /// already there; this is for when the program ITSELF changed (a
    /// live source editor, one debounced edit at a time).
    ///
    /// Reuses this `DomSession`'s existing `Shared` -- and therefore its
    /// one `bind_resize_listener` closure from construction -- instead of
    /// the caller constructing a new `DomSession` per edit. That distinction
    /// matters here specifically because this module's listeners are never
    /// cleaned up (see the module doc comment: "fine for a session that
    /// lives as long as the page, ONE LISTENER TOTAL per DomSession") --
    /// a fresh `DomSession` per keystroke would leak one more `window`
    /// resize listener per edit, `reload` keeps it at the documented
    /// one-per-session baseline no matter how many edits happen.
    #[cfg(feature = "interpreter")]
    pub fn reload(&mut self, source: &str, ui_fn_name: &str) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        match InnerSession::new_with_natives(source, ui_fn_name, Default::default(), &shared.natives) {
            Ok(session) => *shared.session.borrow_mut() = boxed(session),
            Err(e) => return Some(e),
        }
        if shared.nested {
            apply_app_css(&shared, "tint-preview-css");
        }
        sync_viewport_width(&shared);
        let tree = match shared.session.borrow_mut().render() {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }

    /// Replaces the current program with a serialized Tint bytecode blob.
    #[cfg(feature = "interpreter")]
    #[cfg(feature = "bytecode")]
    pub fn reload_bytecode(&mut self, bytes: &[u8], ui_fn_name: &str) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        match InnerSession::from_bytecode_with_natives(bytes, ui_fn_name, &shared.natives) {
            Ok(session) => *shared.session.borrow_mut() = boxed(session),
            Err(e) => return Some(e),
        }
        if shared.nested {
            apply_app_css(&shared, "tint-preview-css");
        }
        sync_viewport_width(&shared);
        let tree = match shared.session.borrow_mut().render() {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }
}

#[derive(serde::Serialize)]
struct HttpRequestJson {
    id: u64,
    method: String,
    url: String,
}

impl DomSession {
    /// Shared constructor tail. A `nested` session (inside a `Preview`) never
    /// touches the host page: no title/body/history, no keyboard capture.
    fn from_inner(
        session: Result<Box<dyn SessionBackend>, String>,
        container_id: &str,
        nested: bool,
        natives: Vec<(String, tint_runtime::ui_session::NativeFn)>,
    ) -> DomSession {
        match session {
            Ok(session) => {
                let shared = Rc::new(Shared {
                    session: RefCell::new(session),
                    container_id: container_id.to_string(),
                    resize_timeout: Cell::new(None),
                    frame_handler: RefCell::new(None),
                    raf_active: Cell::new(false),
                    key_down_handler: RefCell::new(None),
                    key_up_handler: RefCell::new(None),
                    key_down_bound: Cell::new(false),
                    pointer_move_handler: RefCell::new(None),
                    pointer_up_handler: RefCell::new(None),
                    pointer_bound: Cell::new(false),
                    dragging: Cell::new(false),
                    nested,
                    previews: RefCell::new(std::collections::HashMap::new()),
                    preview_seq: Cell::new(0),
                    key_up_bound: Cell::new(false),
                    click_bound: Cell::new(false),
                    life_bound: Cell::new(false),
                    gesture_bound: Cell::new(false),
                    hotkey_bound: Cell::new(false),
                    last_frame_time: Cell::new(None),
                    timers: RefCell::new(Vec::new()),
                    natives,
                    retained: RefCell::new(None),
                });
                bind_resize_listener(&shared);
                if nested {
                    apply_app_css(&shared, "tint-preview-css");
                }
                if !nested {
                    #[cfg(feature = "interpreter")]
                    install_deferred_runner(&shared);
                    apply_app_meta(&shared);
                    bind_navigation(&shared);
                }
                DomSession {
                    shared: Some(shared),
                    init_error: None,
                }
            }
            Err(e) => DomSession {
                shared: None,
                init_error: Some(e),
            },
        }
    }

    #[cfg(feature = "interpreter")]
    pub(crate) fn new_nested(source: &str, ui_fn_name: &str, container_id: &str) -> DomSession {
        dom_tree_shape();
        Self::from_inner(InnerSession::new(source, ui_fn_name).map(boxed), container_id, true, Vec::new())
    }
}

#[cfg(feature = "interpreter")]
fn boxed(session: InnerSession) -> Box<dyn SessionBackend> {
    Box::new(session)
}
