//! Direct-DOM rendering backend for `UiSession`.
//!
//! Each render rebuilds the container subtree instead of diffing it. Event
//! closures are intentionally leaked by `Closure::forget`, which is acceptable
//! for the sandbox but should be addressed before using this in a long-lived app.

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
