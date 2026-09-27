// tint-wasm/src/dom.rs
//
// A second rendering backend for the same UI tree tint_runtime::ui_session
// already produces, alongside the existing Svelte-based one (UiSession in
// lib.rs: hands a serialized Vec<UiRenderNode> to JS, UiPreviewNode.svelte
// turns it into DOM). This one skips JS/Svelte entirely -- Rust calls
// web-sys DOM APIs directly to build real elements from the tree.
//
// Deliberately NOT a diffing renderer: every render (initial mount, or
// after a click/hover_in/hover_out dispatch) tears down the container's
// children and rebuilds the whole subtree from scratch. That matches the
// existing Svelte path's own behavior (Preview.svelte swaps in the whole
// returned tree on every dispatch, no reconciliation) -- see
// tint_runtime::ui_session's doc comment. A real diff/patch pass (to keep
// focus/scroll/animation state across renders) is future work.
//
// Known cost of "rebuild + web-sys event closures": every build_node call
// that attaches a listener does `closure.forget()`, which is the standard
// wasm-bindgen idiom for a closure that must outlive the function that
// created it -- but it also means the closure's memory is never freed,
// even once its element is torn down by the next rebuild. Each click/hover
// therefore leaks the previous render's closures. Fine for a sandbox/demo
// session; not something to ship in a long-running app without adding
// real diffing (which reuses elements/listeners instead of recreating
// them) first.
//
// Additive: UiSession (the JS-facing, Svelte-driving type in lib.rs) is
// untouched. DomSession is a separate, new wasm-bindgen export the
// sandbox can opt into independently.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use wasm_bindgen::closure::Closure;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{Document, Element};

use tint_runtime::ui::render::UiRenderNode;
use tint_runtime::ui_session::UiSession as InnerSession;

/// Shared state a click/hover closure needs: the live session (`state`
/// lives inside it) and where in the DOM to rebuild. Cheap to clone
/// (it's an `Rc`) so every closure can hold its own copy.
struct Shared {
    session: RefCell<InnerSession>,
    container_id: String,
    /// Handle from `window.setTimeout`, used to debounce the `resize`
    /// listener bound in `bind_resize_listener` -- a resize storm
    /// (dragging a window edge) should trigger one rebuild after it
    /// settles, not one per event.
    resize_timeout: Cell<Option<i32>>,
    /// The `frame||handler` name from the most recently mounted tree, if
    /// any -- refreshed on every `mount_tree` call (see
    /// `find_frame_handler`) so the running rAF loop (see
    /// `start_frame_loop`) always dispatches whatever the CURRENT tree
    /// asked for, not whatever was true when the loop started.
    frame_handler: RefCell<Option<String>>,
    /// Whether `start_frame_loop` has already scheduled its
    /// `requestAnimationFrame` chain for this session. A `ui fn` declares
    /// `frame||` at most once in practice, and the loop just keeps
    /// rescheduling itself once started (see `start_frame_loop`), so this
    /// only needs to gate the FIRST scheduling.
    raf_active: Cell<bool>,
    key_down_handler: RefCell<Option<String>>,
    key_up_handler: RefCell<Option<String>>,
    key_down_bound: Cell<bool>,
    key_up_bound: Cell<bool>,
    /// The rAF timestamp (ms, from `performance.now()`) the loop last ran
    /// at, used to compute the real elapsed `dt` (seconds) handed to the
    /// `frame||` handler. `None` on the very first frame, which reports
    /// `dt = 0` rather than a bogus jump from page load.
    last_frame_time: Cell<Option<f64>>,
}

/// Direct-DOM counterpart to `UiSession` in lib.rs. Wraps the exact same
/// `tint_runtime::ui_session::UiSession` (so `state`/`click||`/
/// `hover_in||`/`hover_out||` all behave identically), but instead of
/// handing JS a tree to render, this builds real DOM nodes into
/// `container_id` itself.
///
/// Construction never touches the DOM and can't fail loudly -- same
/// convention as `UiSession::new` (bad source / unknown `ui_fn_name` is
/// stored, not thrown). Call `rerender()` once after constructing to do
/// the first paint, exactly like `UiSession::new` + `.tree()`.
#[wasm_bindgen]
pub struct DomSession {
    shared: Option<Rc<Shared>>,
    init_error: Option<String>,
}

include!("session.rs");
include!("helpers.rs");
include!("render.rs");
