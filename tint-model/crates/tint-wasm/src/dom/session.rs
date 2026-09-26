#[wasm_bindgen]
impl DomSession {
    #[wasm_bindgen(constructor)]
    pub fn new(source: &str, ui_fn_name: &str, container_id: &str) -> DomSession {
        match InnerSession::new(source, ui_fn_name) {
            Ok(session) => {
                let shared = Rc::new(Shared {
                    session: RefCell::new(session),
                    container_id: container_id.to_string(),
                    resize_timeout: Cell::new(None),
                });
                bind_resize_listener(&shared);
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
        mount_tree(&tree, &shared)
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
        mount_tree(&tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
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
    pub fn reload(&mut self, source: &str, ui_fn_name: &str) -> Option<String> {
        let shared = match &self.shared {
            Some(s) => s.clone(),
            None => return self.init_error.clone(),
        };
        match InnerSession::new(source, ui_fn_name) {
            Ok(session) => *shared.session.borrow_mut() = session,
            Err(e) => return Some(e),
        }
        sync_viewport_width(&shared);
        let tree = match shared.session.borrow_mut().render() {
            Ok(tree) => tree,
            Err(e) => return Some(e),
        };
        mount_tree(&tree, &shared)
            .err()
            .map(|e| js_error_to_string(&e))
    }
}
