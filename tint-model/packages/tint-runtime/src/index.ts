/**
 * Public browser-facing API for the prebuilt Tint runtime.
 *
 * The package owns the WASM lifecycle. Applications provide Tint source and
 * a DOM element; they never invoke wasm-pack or rebuild the Tint runtime.
 * The generated wasm-bindgen module is copied next to this file when the
 * runtime package is assembled for release.
 */

import init, { app_assets, app_assets_bytecode, compile_bytecode, DomSession } from "../tint_wasm.js";

export type TintEntry = string;

export interface TintCallbackContext {
  event: Event;
  element: Element;
}

export type TintCallback = (context: TintCallbackContext) => void | Promise<void>;

/** A host function callable from `.tn` as `name(...)`. Synchronous; plain data in and out. */
export type TintNative = (...args: any[]) => unknown;

/** What a component factory may return. Both methods are optional. */
export interface TintComponentInstance {
  /** Called when the Tint `props||{...}` value changes. */
  update?(props: any): void;
  /** Called when the node leaves the tree, the component name changes, or the app unmounts. */
  destroy?(): void;
}

/**
 * Mounts a host component into a Tint node declared as
 * `Host { component||"Name" props||{ ... } }`. `element` is owned by Tint
 * (keep it childless in the Tint source); render into it, and keep your own
 * resources behind `update`/`destroy`.
 */
export type TintComponent = (
  element: HTMLElement,
  props: any,
) => TintComponentInstance | void;

export interface TintMountOptions {
  entry?: TintEntry;
  callbacks?: Record<string, TintCallback>;
  /**
   * Host functions callable from Tint. Merged over the exports of the modules
   * named by `app { js::"./x.js" }` (these win on a name clash).
   */
  natives?: Record<string, TintNative>;
  /** Host components by name, for `component||"Name"` nodes. */
  components?: Record<string, TintComponent>;
  /** What `js::`/`css::` paths in `app { }` resolve against. Default: the page URL. */
  baseUrl?: string | URL;
}

export interface TintApp {
  reload(source: string, options?: TintMountOptions): string | undefined;
  dispatch(handler: string): string | undefined;
  dispatchFrame(handler: string, dt: number): string | undefined;
  unmount(): void;
}

export type TintBytecode = Uint8Array;

let runtimeReady: Promise<unknown> | undefined;

function ensureRuntime(): Promise<unknown> {
  runtimeReady ??= init();
  return runtimeReady;
}

function ensureContainer(element: Element): string {
  if (element.id) return element.id;
  const id = `tint-${Math.random().toString(36).slice(2)}`;
  element.id = id;
  return id;
}

interface AppAssets {
  js: string[];
  css: string[];
}

/**
 * Loads the files an `app { js::"..." css::"..." }` block names: stylesheets
 * become `<link>`s, and every function exported by a JS module is a native.
 */
async function loadAssets(
  assets: AppAssets | null,
  options: TintMountOptions,
): Promise<Record<string, TintNative>> {
  const base = options.baseUrl ?? document.baseURI;
  const natives: Record<string, TintNative> = {};
  for (const href of assets?.css ?? []) {
    const url = new URL(href, base).href;
    if (document.querySelector(`link[data-tint-css="${CSS.escape(url)}"]`)) continue;
    const link = document.createElement("link");
    link.rel = "stylesheet";
    link.href = url;
    link.dataset.tintCss = url;
    document.head.append(link);
  }
  for (const src of assets?.js ?? []) {
    const module = await import(/* @vite-ignore */ new URL(src, base).href);
    for (const [name, value] of Object.entries(module)) {
      if (typeof value === "function") natives[name] = value as TintNative;
    }
  }
  return Object.assign(natives, options.natives);
}

/**
 * Mount a Tint source string into an existing DOM element.
 *
 * `source` is intentionally a string in this first API slice. A Vite plugin
 * can later turn `import App from "./App.tn"` into this same call without
 * changing the runtime contract.
 */
export async function mount(
  source: string,
  element: Element,
  options: TintMountOptions = {},
): Promise<TintApp> {
  await ensureRuntime();

  const entry = options.entry ?? "App";
  const containerId = ensureContainer(element);
  const natives = await loadAssets(app_assets(source), options);
  const session = new DomSession(source, entry, containerId, natives);
  return mountSession(session, element, entry, options.callbacks, options.components);
}

/** Compile source once and return the versioned Tint program blob. */
export async function compile(source: string): Promise<TintBytecode> {
  await ensureRuntime();
  return compile_bytecode(source);
}

/** Mount a previously compiled Tint program through the same runtime. */
export async function mountBytecode(
  bytecode: TintBytecode,
  element: Element,
  options: TintMountOptions = {},
): Promise<TintApp> {
  await ensureRuntime();

  const entry = options.entry ?? "App";
  const containerId = ensureContainer(element);
  const natives = await loadAssets(app_assets_bytecode(bytecode), options);
  const session = DomSession.from_bytecode(bytecode, entry, containerId, natives);
  return mountSession(session, element, entry, options.callbacks, options.components);
}

/**
 * Keeps host components in step with the `data-tint-component` /
 * `data-tint-props` attributes the runtime writes: mount on first sight,
 * `update` when props change, `destroy` when the element goes away. A
 * MutationObserver sees every render path (clicks, timers, reload).
 */
function syncComponents(
  root: Element,
  components: Record<string, TintComponent>,
): () => void {
  interface Live { name: string; raw: string | null; instance: TintComponentInstance | void }
  const live = new Map<Element, Live>();
  const warned = new Set<string>();
  const parse = (raw: string | null) => {
    if (raw === null) return undefined;
    try { return JSON.parse(raw); } catch { return undefined; }
  };
  const guard = (what: string, fn: () => void) => {
    try { fn(); } catch (error) { console.error(`tint: component ${what} failed`, error); }
  };
  const drop = (element: Element, entry: Live) => {
    live.delete(element);
    guard(`"${entry.name}" destroy`, () => entry.instance?.destroy?.());
  };

  const sync = () => {
    for (const [element, entry] of [...live]) {
      if (!root.contains(element) || element.getAttribute("data-tint-component") !== entry.name) {
        drop(element, entry);
      }
    }
    for (const element of root.querySelectorAll<HTMLElement>("[data-tint-component]")) {
      const name = element.getAttribute("data-tint-component")!;
      const raw = element.getAttribute("data-tint-props");
      const entry = live.get(element);
      if (!entry) {
        const factory = components[name];
        if (!factory) {
          if (!warned.has(name)) {
            warned.add(name);
            console.warn(`tint: no component registered for "${name}"`);
          }
          continue;
        }
        const created: Live = { name, raw, instance: undefined };
        live.set(element, created);
        guard(`"${name}" mount`, () => { created.instance = factory(element, parse(raw)); });
      } else if (entry.raw !== raw) {
        entry.raw = raw;
        guard(`"${name}" update`, () => entry.instance?.update?.(parse(raw)));
      }
    }
  };

  const observer = new MutationObserver(sync);
  observer.observe(root, {
    childList: true,
    subtree: true,
    attributes: true,
    attributeFilter: ["data-tint-component", "data-tint-props"],
  });
  sync();
  return () => {
    observer.disconnect();
    for (const [element, entry] of [...live]) drop(element, entry);
  };
}

function mountSession(
  session: DomSession,
  element: Element,
  entry: string,
  callbacks: Record<string, TintCallback> = {},
  components: Record<string, TintComponent> = {},
): TintApp {
  const initialError = session.rerender();
  if (initialError) throw new Error(initialError);
  const stopComponents = syncComponents(element, components);

  const onHostClick = (event: Event) => {
    const target = event.target instanceof Element
      ? event.target.closest("[data-tint-js]")
      : null;
    if (!target || !element.contains(target)) return;
    const name = target.getAttribute("data-tint-js");
    if (!name) return;
    const callback = callbacks[name];
    if (!callback) return;
    void callback({ event, element: target });
  };
  element.addEventListener("click", onHostClick, true);

  let mounted = true;
  return {
    reload(nextSource, nextOptions = {}) {
      if (!mounted) return "Tint app is unmounted";
      const error = session.reload(nextSource, nextOptions.entry ?? entry);
      return error ?? undefined;
    },
    dispatch(handler) {
      if (!mounted) return "Tint app is unmounted";
      const error = session.dispatch(handler);
      return error ?? undefined;
    },
    dispatchFrame(handler, dt) {
      if (!mounted) return "Tint app is unmounted";
      const error = session.dispatch_frame(handler, dt);
      return error ?? undefined;
    },
    unmount() {
      if (!mounted) return;
      mounted = false;
      stopComponents();
      element.removeEventListener("click", onHostClick, true);
      element.replaceChildren();
    },
  };
}
