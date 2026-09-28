/**
 * Public browser-facing API for the prebuilt Tint runtime.
 *
 * The package owns the WASM lifecycle. Applications provide Tint source and
 * a DOM element; they never invoke wasm-pack or rebuild the Tint runtime.
 * The generated wasm-bindgen module is copied next to this file when the
 * runtime package is assembled for release.
 */

import init, { compile_bytecode, DomSession } from "../tint_wasm.js";

export type TintEntry = string;

export interface TintMountOptions {
  entry?: TintEntry;
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
  const session = new DomSession(source, entry, containerId);
  return mountSession(session, element, entry);
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
  const session = DomSession.from_bytecode(bytecode, entry, containerId);
  return mountSession(session, element, entry);
}

function mountSession(
  session: DomSession,
  element: Element,
  entry: string,
): TintApp {
  const initialError = session.rerender();
  if (initialError) throw new Error(initialError);

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
      element.replaceChildren();
    },
  };
}
