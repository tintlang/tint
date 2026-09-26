import "./app.css";
import initWasm, {
  check,
  run,
  tint_version,
  UiSession,
  DomSession,
} from "../pkg-web/tint_wasm.js";
import shellSource from "./sandbox/index.tn";
import demoSource from "../../examples/ui_app.tn";

import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching, indentOnInput, syntaxHighlighting, HighlightStyle } from "@codemirror/language";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { tags } from "@lezer/highlight";
import { tintLanguage } from "./lib/tintLanguage.js";

const root = document.querySelector("#root");
let shellSession;
let previewSession;
let editorView;
let lastCodePanel;
let updateTimer;
let shellTheme = "dark";

const darkColors = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword], color: "#a29bfe" },
  { tag: tags.string, color: "#8fd19e" },
  { tag: [tags.number, tags.atom, tags.bool], color: "#f5a623" },
  { tag: tags.typeName, color: "#7db8ff" },
  { tag: tags.function(tags.variableName), color: "#7db8ff" },
  { tag: tags.comment, color: "#7a7a7a", fontStyle: "italic" },
  { tag: [tags.operator, tags.punctuation], color: "#9a9a9a" },
]);

const lightColors = HighlightStyle.define([
  { tag: [tags.keyword, tags.operatorKeyword], color: "#6c5ce7" },
  { tag: tags.string, color: "#218739" },
  { tag: [tags.number, tags.atom, tags.bool], color: "#b45309" },
  { tag: tags.typeName, color: "#1769aa" },
  { tag: tags.function(tags.variableName), color: "#1769aa" },
  { tag: tags.comment, color: "#777777", fontStyle: "italic" },
  { tag: [tags.operator, tags.punctuation], color: "#777777" },
]);

const editorTheme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "transparent", color: "var(--text)" },
  ".cm-content": { fontFamily: "ui-monospace, SFMono-Regular, Menlo, Consolas, monospace", fontSize: "13px", lineHeight: "1.7", padding: "0", caretColor: "var(--text)" },
  ".cm-gutters": { backgroundColor: "var(--editor-bg)", color: "var(--dim)", border: "none" },
  ".cm-activeLine": { backgroundColor: "rgba(128,128,128,.08)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" },
  "&.cm-focused": { outline: "none" },
  ".cm-scroller": { overflow: "auto" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "var(--selection)" },
}, { dark: true });

const themeCompartment = new Compartment();
const syntaxCompartment = new Compartment();

function isDark() {
  return document.documentElement.dataset.theme !== "light";
}

function setEditorTheme() {
  if (!editorView) return;
  const dark = isDark();
  editorView.dispatch({ effects: [
    themeCompartment.reconfigure(editorTheme),
    syntaxCompartment.reconfigure(syntaxHighlighting(dark ? darkColors : lightColors)),
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
  if (output) {
    output.textContent = message;
    return;
  }

  // A shell parse/render error happens before OutputPanel exists. Keep the
  // failure visible instead of leaving the app on a blank white page.
  let errorPanel = root.querySelector(".sandbox-bootstrap-error");
  if (!errorPanel) {
    errorPanel = document.createElement("pre");
    errorPanel.className = "sandbox-bootstrap-error";
    root.replaceChildren(errorPanel);
  }
  errorPanel.textContent = message || "Tint sandbox failed to render";
}

function dispatchPreview(handler) {
  if (!previewSession) return;
  const result = previewSession.dispatch(handler);
  if (result?.error) logEvent(`${handler}: ${result.error}`);
  else renderPreview(result?.tree || []);
}

function updatePreview(source) {
  if (!previewSession) return;
  // The editor holds a full program (plain `fn`s plus a `ui fn App()`),
  // exactly like demoSource / what check()/run() already compile -- not
  // a bare UI-node snippet, so it must be reloaded as-is with "App" as
  // the entry point, the same way `new UiSession(demoSource, "App")`
  // constructed this session in the first place. Wrapping it in a second
  // `ui fn Preview() { ... }` (the old behavior here) nested those `fn`/
  // `ui fn` declarations inside a UI body, which never parses, and
  // `UiSession` had no `reload` method at all until now -- so every edit
  // silently threw before ever reaching this line, leaving the preview
  // stuck on whatever last rendered.
  const result = previewSession.reload(source, "App");
  if (result?.error) logEvent(result.error);
  else renderPreview(result?.tree || []);
}

function mountEditor(panel) {
  if (!editorView) {
    editorView = new EditorView({
      state: EditorState.create({
        doc: demoSource,
        extensions: [
          lineNumbers(), history(), highlightActiveLine(), bracketMatching(), closeBrackets(),
          indentOnInput(), tintLanguage(), syntaxHighlighting(darkColors),
          themeCompartment.of(editorTheme), syntaxCompartment.of(syntaxHighlighting(darkColors)),
          keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
          EditorView.updateListener.of((update) => {
            if (!update.docChanged) return;
            clearTimeout(updateTimer);
            updateTimer = setTimeout(() => updatePreview(update.state.doc.toString()), 300);
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
  setEditorTheme();
  // Theme/file events can replace the shell subtree. Keep the UiSession
  // alive and paint its current tree into the newly-created mount.
  if (previewSession) renderPreview(previewSession.tree()?.tree || []);
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
shellSession = new DomSession(shellSource, "Sandbox", "root");
const shellError = shellSession.rerender();
if (shellError) logEvent(shellError);
previewSession = new UiSession(demoSource, "App");
renderPreview(previewSession.tree()?.tree || []);
remount();
