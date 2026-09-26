import "./app.css";
import initWasm, {
  check,
  run,
  tint_version,
  UiSession,
  DomSession,
} from "../pkg-web/tint_wasm.js";
import shellSource from "./sandbox.tn";
import demoSource from "../../examples/ui_app.tn";

import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching, indentOnInput, syntaxHighlighting } from "@codemirror/language";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { tintLanguage } from "./lib/tintLanguage.js";
import { darkHighlightColors, lightHighlightColors } from "./lib/tintHighlight.js";
import { loadStoredTheme, storeTheme } from "./lib/themeStorage.js";

const root = document.querySelector("#root");
let shellSession;
let previewSession;
let editorView;
let lastCodePanel;
let lastPreviewMount;
let updateTimer;
let shellTheme = "dark";

const CODE_STORAGE_KEY = "tint-sandbox-code";

function loadStoredCode() {
  try {
    return localStorage.getItem(CODE_STORAGE_KEY);
  } catch {
    return null;
  }
}

function storeCode(code) {
  try {
    localStorage.setItem(CODE_STORAGE_KEY, code);
  } catch {}
}

const PREVIEW_MIN_PCT = 25;
const PREVIEW_MAX_PCT = 50;

function currentPreviewPct(workspace, panel) {
  const workspaceWidth = workspace.getBoundingClientRect().width;
  if (!workspaceWidth) return null;
  return (panel.getBoundingClientRect().width / workspaceWidth) * 100;
}

function setPreviewPct(pct) {
  const clamped = Math.min(PREVIEW_MAX_PCT, Math.max(PREVIEW_MIN_PCT, pct));
  document.documentElement.style.setProperty("--preview-w", `${clamped}%`);
}

function bindResizer(resizer, workspace, panel) {
  resizer.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    const workspaceWidth = workspace.getBoundingClientRect().width;
    const startPct = currentPreviewPct(workspace, panel);
    if (!workspaceWidth || startPct == null) return;
    const startX = event.clientX;
    resizer.setPointerCapture(event.pointerId);
    document.body.classList.add("tint-resizing");

    const onMove = (moveEvent) => {
      const deltaPct = ((startX - moveEvent.clientX) / workspaceWidth) * 100;
      setPreviewPct(startPct + deltaPct);
    };
    const onUp = () => {
      resizer.removeEventListener("pointermove", onMove);
      document.body.classList.remove("tint-resizing");
    };
    resizer.addEventListener("pointermove", onMove);
    resizer.addEventListener("pointerup", onUp, { once: true });
  });
}

const editorTheme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "transparent", color: "inherit" },
  ".cm-content": { fontFamily: "inherit", fontSize: "inherit", lineHeight: "1.7", padding: "0" },
  ".cm-gutters": { backgroundColor: "#141414", color: "inherit", border: "none" },
  ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.04)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { overflow: "auto" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "rgba(108,92,231,0.35)" },
}, { dark: true });

const editorLightTheme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "transparent", color: "inherit" },
  ".cm-content": { fontFamily: "inherit", fontSize: "inherit", lineHeight: "1.7", padding: "0" },
  ".cm-gutters": { backgroundColor: "#f5f5f5", color: "inherit", border: "none" },
  ".cm-activeLine": { backgroundColor: "rgba(0,0,0,0.04)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { overflow: "auto" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "rgba(108,92,231,0.18)" },
}, { dark: false });

const themeCompartment = new Compartment();
const syntaxCompartment = new Compartment();

function isDark() {
  return document.documentElement.dataset.theme !== "light";
}

function setEditorTheme() {
  if (!editorView) return;
  const dark = isDark();
  editorView.dispatch({ effects: [
    themeCompartment.reconfigure(dark ? editorTheme : editorLightTheme),
    syntaxCompartment.reconfigure(syntaxHighlighting(dark ? darkHighlightColors : lightHighlightColors)),
  ] });
}

function styleText(node, hovering = false) {
  const values = new Map(node.style || []);
  if (hovering) for (const [key, value] of node.hover_style || []) values.set(key, value);
  return [...values].map(([key, value]) => `${key}: ${value}`).join("; ");
}

function renderPreviewNode(node, onAction, onEvent) {
  const hasSvg = node.svg != null;
  const element = hasSvg
    ? document.createElement("div")
    : node.route
      ? document.createElement("a")
      : (node.tag === "Button" || node.tag === "MenuItem" || node.on_click)
      ? document.createElement("button")
      : (node.tag === "Text" && node.text != null && !(node.children || []).length)
        ? document.createElement("span")
        : document.createElement("div");

  element.className = "tint-ui-node";
  element.dataset.tag = node.tag;
  if (node.route) element.setAttribute("href", node.route);
  element.style.cssText = styleText(node);
  for (const [key, value] of node.attrs || []) element.dataset[key] = value;
  if (hasSvg) element.innerHTML = node.svg;
  else if (node.text != null) element.append(document.createTextNode(node.text));
  for (const child of node.children || []) element.append(renderPreviewNode(child, onAction, onEvent));

  let hovering = false;
  const refreshHover = (value) => {
    hovering = value;
    element.style.cssText = styleText(node, hovering);
  };
  element.addEventListener("mouseenter", () => {
    refreshHover(true);
    if (node.on_hover_enter) {
      onAction(node.on_hover_enter);
      onEvent(`hover in -> ${node.on_hover_enter}()`);
    }
  });
  element.addEventListener("mouseleave", () => {
    refreshHover(false);
    if (node.on_hover_leave) {
      onAction(node.on_hover_leave);
      onEvent(`hover out -> ${node.on_hover_leave}()`);
    }
  });
  if (node.on_click) element.addEventListener("click", () => {
    onAction(node.on_click);
    onEvent(`click -> ${node.on_click}()`);
  });
  return element;
}

function renderPreview(tree) {
  const mount = root.querySelector('[data-tag="PreviewMount"]');
  if (!mount) return;
  mount.replaceChildren();
  if (!tree?.length) {
    const empty = document.createElement("div");
    empty.className = "preview-empty";
    empty.textContent = "No UI tree returned";
    mount.append(empty);
    return;
  }
  const canvas = document.createElement("div");
  canvas.className = "visual-preview";
  for (const node of tree) canvas.append(renderPreviewNode(node, dispatchPreview, logEvent));
  mount.append(canvas);
}

function logEvent(message) {
  const output = root.querySelector('[data-tag="OutputText"]');
  if (output) output.textContent = message;
}

function dispatchPreview(handler) {
  if (!previewSession) return;
  const result = previewSession.dispatch(handler);
  if (result?.error) logEvent(`${handler}: ${result.error}`);
  else renderPreview(result?.tree || []);
}

function updatePreview(source) {
  if (!previewSession) return;
  const result = previewSession.reload(`ui fn Preview() { ${source} }`, "Preview");
  if (result?.error) logEvent(result.error);
  else renderPreview(result?.tree || []);
}

function mountEditor(panel) {
  if (!editorView) {
    editorView = new EditorView({
      state: EditorState.create({
        doc: loadStoredCode() ?? demoSource,
        extensions: [
          lineNumbers(), history(), highlightActiveLine(), bracketMatching(), closeBrackets(),
          indentOnInput(), tintLanguage(),
          themeCompartment.of(editorTheme), syntaxCompartment.of(syntaxHighlighting(darkHighlightColors)),
          keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;
            clearTimeout(updateTimer);
            updateTimer = setTimeout(() => {
              const source = update.state.doc.toString();
              storeCode(source);
              updatePreview(source);
            }, 300);
          }),
        ],
      }),
    });
    editorView.dom.classList.add("tint-editor");
    editorView.dom.addEventListener("keydown", (event) => {
      if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
        event.preventDefault();
        runCode();
      }
    });
  }
  if (panel !== lastCodePanel) {
    lastCodePanel = panel;
    panel.replaceChildren(editorView.dom);
  }
}

function showResult(result, mode) {
  const output = root.querySelector('[data-tag="OutputText"]');
  if (!output) return;
  if (result?.error) output.textContent = result.error;
  else output.textContent = mode === "check"
    ? `✓ Check passed · ${result?.item_count ?? 0} items parsed`
    : `✓ Run completed · ${result?.value ?? "()"}`;
}

function checkCode() {
  showResult(check(editorView?.state.doc.toString() || ""), "check");
}

function runCode() {
  showResult(run(editorView?.state.doc.toString() || "", "main"), "run");
}

function renderShellTheme(theme) {
  shellTheme = theme;
  document.documentElement.dataset.theme = theme;
  storeTheme(theme);
  const source = shellSource.replace('state theme = "dark"', `state theme = "${theme}"`);
  shellSession?.free();
  shellSession = new DomSession(source, "Sandbox", "root");
  const error = shellSession.rerender();
  if (error) logEvent(error);
  if (previewSession) renderPreview(previewSession.tree()?.tree || []);
}

function remount() {
  const panel = root.querySelector('[data-tag="CodePanel"]');
  if (panel) mountEditor(panel);
  for (const button of root.querySelectorAll('[data-tag="DarkBtn"], [data-tag="LightBtn"]')) {
    if (button.dataset.hostThemeBound) continue;
    button.dataset.hostThemeBound = "true";
    button.onclick = () => renderShellTheme(button.dataset.tag === "LightBtn" ? "light" : "dark");
  }
  const checkButton = root.querySelector('[data-tag="CheckBtn"]');
  const runButton = root.querySelector('[data-tag="RunBtn"]');
  if (checkButton) checkButton.onclick = checkCode;
  if (runButton) runButton.onclick = runCode;
  const workspace = root.querySelector('[data-tag="Workspace"]');
  const resizer = root.querySelector('[data-tag="Resizer"]');
  const previewPanel = root.querySelector('[data-tag="PreviewPanel"]');
  if (workspace && resizer && previewPanel && !resizer.dataset.resizeBound) {
    resizer.dataset.resizeBound = "true";
    bindResizer(resizer, workspace, previewPanel);
  }
  // CheckBtn/RunBtn also carry a click||check_code / click||run_code
  // binding in the .tn shell source (see sandbox.tn) so DomSession treats
  // them as a real dispatch and rebuilds the clicked subtree -- even
  // though check_code/run_code are no-op Tint fns and the actual
  // check/run work happens in checkCode()/runCode() above. That rebuild
  // can recreate PreviewPanel/PreviewMount as fresh, empty DOM nodes,
  // orphaning whatever renderPreview() had appended into the old ones
  // (renderShellTheme() already re-renders the preview after ITS OWN
  // intentional rebuild; this covers the same case for an implicit one).
  const previewMount = root.querySelector('[data-tag="PreviewMount"]');
  if (previewMount && previewMount !== lastPreviewMount) {
    lastPreviewMount = previewMount;
    if (previewSession) renderPreview(previewSession.tree()?.tree || []);
  }
  setEditorTheme();
}

// Capture before DomSession's own event handler rebuilds the subtree. The
// renderer intentionally replaces the clicked node after a Tint dispatch, so
// a bubbling listener would otherwise be attached to a DOM branch that has
// already been removed.
document.addEventListener("click", (event) => {
  const tag = event.target.closest("[data-tag]")?.dataset.tag;
  if (tag === "CheckBtn") checkCode();
  if (tag === "RunBtn") runCode();
  // Keep theme switching explicit at the host boundary as well. DomSession
  // binds Tint events itself, but this makes the app shell resilient while
  // its DOM is being replaced after a stateful dispatch.
  if (tag === "DarkBtn") renderShellTheme("dark");
  if (tag === "LightBtn") renderShellTheme("light");
}, true);

new MutationObserver(remount).observe(root, { childList: true, subtree: true });

await initWasm();
document.title = `Tint Sandbox · ${tint_version()}`;
if (new URLSearchParams(window.location.search).get("route") === "/sandbox") {
  window.history.replaceState({}, "", "/sandbox");
}
const initialTheme = loadStoredTheme();
shellTheme = initialTheme;
document.documentElement.dataset.theme = initialTheme;
const initialShellSource = initialTheme === "dark"
  ? shellSource
  : shellSource.replace('state theme = "dark"', `state theme = "${initialTheme}"`);
shellSession = new DomSession(initialShellSource, "Sandbox", "root");
const shellError = shellSession.rerender();
if (shellError) logEvent(shellError);
previewSession = new UiSession(demoSource, "App");
renderPreview(previewSession.tree()?.tree || []);
remount();
