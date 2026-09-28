/* tslint:disable */
/* eslint-disable */

/**
 * Direct-DOM counterpart to `UiSession` in lib.rs. Wraps the exact same
 * `tint_runtime::ui_session::UiSession` (so `state`/`click||`/
 * `hover_in||`/`hover_out||` all behave identically), but instead of
 * handing JS a tree to render, this builds real DOM nodes into
 * `container_id` itself.
 *
 * Construction never touches the DOM and can't fail loudly -- same
 * convention as `UiSession::new` (bad source / unknown `ui_fn_name` is
 * stored, not thrown). Call `rerender()` once after constructing to do
 * the first paint, exactly like `UiSession::new` + `.tree()`.
 */
export class DomSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Runs `handler` against this session's persistent state (same
     * dispatch `tint_runtime::ui_session::UiSession::dispatch` already
     * does for the Svelte path), then rebuilds the DOM from the result.
     */
    dispatch(handler: string): string | undefined;
    /**
     * Dispatches a `frame||handler` with elapsed seconds as `dt`.
     */
    dispatch_frame(handler: string, dt: number): string | undefined;
    /**
     * Delivers `(request_id, status, body)` to a Tint HTTP handler and
     * updates the DOM with its result.
     */
    dispatch_http(handler: string, request_id: number, status: number, body: string): string | undefined;
    /**
     * Creates a session from a versioned Tint bytecode blob. The browser
     * host can choose source mode for development and this mode for release
     * artifacts without changing the DOM/session API.
     */
    static from_bytecode(bytes: Uint8Array, ui_fn_name: string, container_id: string): DomSession;
    hydrate_storage(values: any): void;
    constructor(source: string, ui_fn_name: string, container_id: string);
    /**
     * Points this SAME `DomSession` at different source -- re-parsing
     * `source` as `ui_fn_name` into a brand-new inner session and
     * rendering it, in place of the one built at construction time.
     * `dispatch` above replays a click/hover against the program that's
     * already there; this is for when the program ITSELF changed (a
     * live source editor, one debounced edit at a time).
     *
     * Reuses this `DomSession`'s existing `Shared` -- and therefore its
     * one `bind_resize_listener` closure from construction -- instead of
     * the caller constructing a new `DomSession` per edit. That distinction
     * matters here specifically because this module's listeners are never
     * cleaned up (see the module doc comment: "fine for a session that
     * lives as long as the page, ONE LISTENER TOTAL per DomSession") --
     * a fresh `DomSession` per keystroke would leak one more `window`
     * resize listener per edit, `reload` keeps it at the documented
     * one-per-session baseline no matter how many edits happen.
     */
    reload(source: string, ui_fn_name: string): string | undefined;
    /**
     * Replaces the current program with a serialized Tint bytecode blob.
     */
    reload_bytecode(bytes: Uint8Array, ui_fn_name: string): string | undefined;
    /**
     * Re-renders the session's current tree into the container,
     * replacing whatever was there. Returns `Some(error)` instead of
     * throwing; `None` means it went fine.
     */
    rerender(): string | undefined;
    storage_snapshot(): any;
    /**
     * Returns and clears queued `http_get(url)` work as a JS array. The
     * browser host performs fetch and feeds completion to `dispatch_http`.
     */
    take_http_requests(): any;
}

/**
 * A stateful REPL session for the sandbox's terminal panel, unlike
 * `check()`/`run()`/`render_ui()` above which each construct a fresh,
 * throwaway `TintVM` (see this crate's other functions' doc comments and
 * sandbox/README.md's "Known limitations"). Backed by
 * `tint_runtime::repl::ReplSession` -- see that module's doc comment for
 * exactly how persistence works (re-parse-and-rerun-everything, not true
 * incrementality) and why every call is panic-hardened (an interactive
 * REPL hits the IR VM's panic-on-runtime-error paths, e.g. calling an
 * undefined function, as a normal/frequent case, not a rare one).
 */
export class TintRepl {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Evaluate one line/entry against everything defined earlier in this
     * session.
     */
    eval(line: string): any;
    constructor();
    /**
     * Forget every definition made in this session so far.
     */
    reset(): void;
}

/**
 * A stateful UI session for the sandbox's preview pane, backing real
 * `click||`/`hover_in||`/`hover_out||` interactivity -- unlike
 * `render_ui()` above, which builds a fresh, throwaway `TintVM` on every
 * call (fine for "re-render on every keystroke", useless for "remember
 * that the popover is open"), this wraps a single
 * `tint_runtime::ui_session::UiSession` that stays alive for the
 * session's lifetime. See that module's doc comment for exactly how
 * `state` persistence and handler dispatch work under here.
 *
 * Construction can fail (bad source, unknown `ui_fn_name`) without a JS
 * exception -- the failure is stored and reported from `tree()`/
 * `dispatch()` instead, so `new UiSession(...)` itself never throws.
 */
export class UiSession {
    free(): void;
    [Symbol.dispose](): void;
    /**
     * Runs `handler` (a plain `fn`, no args) against this session's
     * persistent state, then returns the rebuilt tree -- see
     * `tint_runtime::ui_session`'s doc comment for why this needs a
     * real, persistent session rather than a fresh `render_ui()` call.
     */
    dispatch(handler: string): any;
    /**
     * Dispatches a Tint frame handler with elapsed seconds as `dt`.
     */
    dispatch_frame(handler: string, dt: number): any;
    /**
     * Delivers an HTTP completion to a Tint handler as
     * `(request_id, status, body)`.
     */
    dispatch_http(handler: string, request_id: number, status: number, body: string): any;
    /**
     * Merges a plain JS object into the VM storage.
     */
    hydrate_storage(values: any): void;
    constructor(source: string, ui_fn_name: string);
    /**
     * Points this SAME `UiSession` at different source -- re-parsing
     * `source` as `ui_fn_name` into a brand-new inner session (this
     * resets `state`, exactly like constructing a new session would)
     * and returning the freshly-rendered tree in the same shape as
     * `tree()`/`dispatch()`. Mirrors `DomSession::reload` for this
     * JS-tree-consuming path: the sandbox's live editor calls this on
     * every debounced edit instead of throwing away and reconstructing
     * the whole `UiSession` (and losing the ability to report a
     * bad-source error the same way `tree()` does) per keystroke.
     */
    reload(source: string, ui_fn_name: string): any;
    /**
     * Reads the VM storage as a plain JS object for localStorage syncing.
     */
    storage_snapshot(): any;
    /**
     * Returns and clears requests queued by Tint's `http_get(url)` calls.
     * The host should perform fetch and later call `dispatch_http`.
     */
    take_http_requests(): any;
    /**
     * The session's current tree (a fresh build, not cached).
     */
    tree(): any;
}

/**
 * Lex + parse `source` only. Mirrors `tint check`.
 */
export function check(source: string): any;

/**
 * Compiles Tint source into the versioned program blob consumed by
 * `DomSession::from_bytecode` and `reload_bytecode`.
 *
 * This is primarily useful to hosts that want to cache compiled programs.
 * Applications may use source mode directly; no bytecode step is required
 * for development.
 */
export function compile_bytecode(source: string): Uint8Array;

/**
 * Parses `source`, runs it, and calls `ui_fn_name` (a `ui fn`, not a
 * plain `fn`) with NO arguments -- same no-arg-only limitation as
 * `run()`/`tint-cli`'s `run` command, for the same reason (no CLI/JS-side
 * argument marshalling yet). Returns the evaluated render tree: tags
 * with resolved text, if{}/for{} directives actually applied, and a
 * best-effort CSS `style` list translated from Tint's modifiers (see
 * tint_runtime::ui::render's module doc comment for exactly what's
 * covered). The sandbox turns this into real DOM elements.
 */
export function render_ui(source: string, ui_fn_name: string): any;

/**
 * Parse, compile to IR, and call `entry` with no arguments. Mirrors
 * `tint run <file> [entry]`. Any output from print()/dbg() inside the
 * program is captured and returned in `output` rather than lost, since
 * there is no real stdout to write to here.
 */
export function run(source: string, entry: string): any;

export function tint_version(): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
    readonly memory: WebAssembly.Memory;
    readonly __wbg_domsession_free: (a: number, b: number) => void;
    readonly __wbg_tintrepl_free: (a: number, b: number) => void;
    readonly __wbg_uisession_free: (a: number, b: number) => void;
    readonly check: (a: number, b: number) => any;
    readonly compile_bytecode: (a: number, b: number) => [number, number, number, number];
    readonly domsession_dispatch: (a: number, b: number, c: number) => [number, number];
    readonly domsession_dispatch_frame: (a: number, b: number, c: number, d: number) => [number, number];
    readonly domsession_dispatch_http: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => [number, number];
    readonly domsession_from_bytecode: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly domsession_hydrate_storage: (a: number, b: any) => [number, number];
    readonly domsession_new: (a: number, b: number, c: number, d: number, e: number, f: number) => number;
    readonly domsession_reload: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly domsession_reload_bytecode: (a: number, b: number, c: number, d: number, e: number) => [number, number];
    readonly domsession_rerender: (a: number) => [number, number];
    readonly domsession_storage_snapshot: (a: number) => any;
    readonly domsession_take_http_requests: (a: number) => any;
    readonly render_ui: (a: number, b: number, c: number, d: number) => any;
    readonly run: (a: number, b: number, c: number, d: number) => any;
    readonly tint_version: () => [number, number];
    readonly tintrepl_eval: (a: number, b: number, c: number) => any;
    readonly tintrepl_new: () => number;
    readonly tintrepl_reset: (a: number) => void;
    readonly uisession_dispatch: (a: number, b: number, c: number) => any;
    readonly uisession_dispatch_frame: (a: number, b: number, c: number, d: number) => any;
    readonly uisession_dispatch_http: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => any;
    readonly uisession_hydrate_storage: (a: number, b: any) => [number, number];
    readonly uisession_new: (a: number, b: number, c: number, d: number) => number;
    readonly uisession_reload: (a: number, b: number, c: number, d: number, e: number) => any;
    readonly uisession_storage_snapshot: (a: number) => any;
    readonly uisession_take_http_requests: (a: number) => any;
    readonly uisession_tree: (a: number) => any;
    readonly wasm_bindgen_27a16a9810de17ac___convert__closures_____invoke___f64______true_: (a: number, b: number, c: number) => void;
    readonly wasm_bindgen_27a16a9810de17ac___convert__closures_____invoke___web_sys_2fd947709365e817___features__gen_KeyboardEvent__KeyboardEvent______true_: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_27a16a9810de17ac___convert__closures_____invoke___web_sys_2fd947709365e817___features__gen_KeyboardEvent__KeyboardEvent______true__30: (a: number, b: number, c: any) => void;
    readonly wasm_bindgen_27a16a9810de17ac___convert__closures_____invoke_______true_: (a: number, b: number) => void;
    readonly __wbindgen_malloc: (a: number, b: number) => number;
    readonly __wbindgen_realloc: (a: number, b: number, c: number, d: number) => number;
    readonly __wbindgen_exn_store: (a: number) => void;
    readonly __externref_table_alloc: () => number;
    readonly __wbindgen_externrefs: WebAssembly.Table;
    readonly __wbindgen_destroy_closure: (a: number, b: number) => void;
    readonly __externref_table_dealloc: (a: number) => void;
    readonly __wbindgen_free: (a: number, b: number, c: number) => void;
    readonly __wbindgen_start: () => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
 * Instantiates the given `module`, which can either be bytes or
 * a precompiled `WebAssembly.Module`.
 *
 * @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
 *
 * @returns {InitOutput}
 */
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
 * If `module_or_path` is {RequestInfo} or {URL}, makes a request and
 * for everything else, calls `WebAssembly.instantiate` directly.
 *
 * @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
 *
 * @returns {Promise<InitOutput>}
 */
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
