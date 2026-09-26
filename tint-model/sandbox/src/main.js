import "./landing.css";
import initWasm, { DomSession } from "../pkg-web/tint_wasm.js";
import landingSource from "./main.tn";
import { Compartment, EditorState } from "@codemirror/state";
import { EditorView, keymap, lineNumbers, highlightActiveLine } from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { bracketMatching, indentOnInput, syntaxHighlighting } from "@codemirror/language";
import { closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { tintLanguage } from "./lib/tintLanguage.js";
import { darkHighlightColors, lightHighlightColors } from "./lib/tintHighlight.js";
import { loadStoredTheme, storeTheme } from "./lib/themeStorage.js";

if (window.location.pathname === "/sandbox") window.location.replace("/app.html?route=/sandbox");

const root = document.getElementById("root");
const PREVIEW_MOUNT_ID = "tint-landing-preview-mount";
const INITIAL_SNIPPET = `Card {
    layout::{ padding::24 }
    paint::{
        radius::20,
        gradient::{ angle::135, from::#6c5ce7, to::#8b7cf0 },
        border::{1, #8b7cf0}
    }
    motion::{ hover::{ scale::1.03, shadow::"0 16px 40px rgba(0,0,0,.35)" } }
    Column {
        layout::{ direction::column }
        Row {
            layout::{ direction::row, gap::12, margin.b::4 }
            Text { text::{12, #d8d2f7} "TOTAL BALANCE" }
            Text { text::{12, bold, white} "VISA" }
        }
        Text { text::{34, bold, white} layout::{ margin.b::28 } "$4,218.90" }
        Text { text::{15, #d8d2f7} layout::{ margin.b::4 } "•••• •••• •••• 4821" }
        Row {
            layout::{ direction::row, gap::12 }
            Text { text::{13, white} "Name Surname" }
            Text { text::{13, #d8d2f7} "12/29" }
        }
    }
}`;

const editorTheme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "transparent", color: "inherit" },
  ".cm-content": { fontFamily: "inherit", fontSize: "inherit", lineHeight: "1.7", padding: "0" },
  ".cm-gutters": { backgroundColor: "transparent", color: "inherit", border: "none" }, ".cm-activeLine": { backgroundColor: "rgba(255,255,255,0.04)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" }, "&.cm-focused": { outline: "none" }, ".cm-scroller": { overflow: "auto" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "rgba(108,92,231,0.35)" },
}, { dark: true });
const editorLightTheme = EditorView.theme({
  "&": { height: "100%", backgroundColor: "transparent", color: "inherit" },
  ".cm-content": { fontFamily: "inherit", fontSize: "inherit", lineHeight: "1.7", padding: "0" },
  ".cm-gutters": { backgroundColor: "transparent", color: "inherit", border: "none" }, ".cm-activeLine": { backgroundColor: "rgba(0,0,0,0.04)" },
  ".cm-activeLineGutter": { backgroundColor: "transparent" }, "&.cm-focused": { outline: "none" }, ".cm-scroller": { overflow: "auto" },
  ".cm-selectionBackground, &.cm-focused .cm-selectionBackground": { backgroundColor: "rgba(108,92,231,0.18)" },
}, { dark: false });

const editorThemeCompartment = new Compartment();
const editorSyntaxCompartment = new Compartment();
let previewSession = null;
let debounceHandle;
const wrapPreviewSource = (body) => `ui fn Preview() {\n${body}\n}`;
function showPreviewError(err) {
  const panel = root.querySelector('[data-tag="PreviewPanel"]');
  if (!panel || !err) return;
  panel.innerHTML = "";
  const msg = document.createElement("div");
  msg.style.cssText = "font: 12px ui-monospace, Menlo, Consolas, monospace; color: #ff8a80; white-space: pre-wrap;";
  msg.textContent = err;
  panel.appendChild(msg);
}
function startPreviewSession(panel, body) {
  panel.id = PREVIEW_MOUNT_ID;
  previewSession?.free();
  previewSession = new DomSession(wrapPreviewSource(body), "Preview", PREVIEW_MOUNT_ID);
  showPreviewError(previewSession.rerender());
}

const initialTheme = loadStoredTheme();
let editorIsDark = initialTheme !== "light";

const editorView = new EditorView({ state: EditorState.create({ doc: INITIAL_SNIPPET, extensions: [
  lineNumbers(), history(), highlightActiveLine(), bracketMatching(), closeBrackets(), indentOnInput(), tintLanguage(),
  editorThemeCompartment.of(editorIsDark ? editorTheme : editorLightTheme), editorSyntaxCompartment.of(syntaxHighlighting(editorIsDark ? darkHighlightColors : lightHighlightColors)),
  keymap.of([...closeBracketsKeymap, ...defaultKeymap, ...historyKeymap, indentWithTab]),
  EditorView.updateListener.of((update) => {
    if (!update.docChanged) return;
    clearTimeout(debounceHandle);
    debounceHandle = setTimeout(() => { if (previewSession) showPreviewError(previewSession.reload(wrapPreviewSource(update.state.doc.toString()), "Preview")); }, 350);
  }),
] }) });
editorView.dom.style.flex = "1 1 auto";
editorView.dom.style.minHeight = "0";
editorView.dom.style.overflow = "hidden";

function syncEditorAppearance() {
  const page = root.querySelector('[data-tag="Page"]');
  if (!page) return;
  const nextIsDark = page.style.backgroundColor === "rgb(10, 10, 10)" || page.style.backgroundColor === "#0a0a0a";
  if (nextIsDark === editorIsDark) return;
  editorIsDark = nextIsDark;
  storeTheme(editorIsDark ? "dark" : "light");
  editorView.dispatch({ effects: [editorThemeCompartment.reconfigure(editorIsDark ? editorTheme : editorLightTheme), editorSyntaxCompartment.reconfigure(syntaxHighlighting(editorIsDark ? darkHighlightColors : lightHighlightColors))] });
}
let lastCodePanel = null;
function remountDemoIfNeeded() {
  const codePanel = root.querySelector('[data-tag="CodePanel"]');
  if (!codePanel || codePanel === lastCodePanel) return;
  lastCodePanel = codePanel;
  codePanel.appendChild(editorView.dom);
  const previewPanel = root.querySelector('[data-tag="PreviewPanel"]');
  if (previewPanel) startPreviewSession(previewPanel, editorView.state.doc.toString());
}
new MutationObserver(() => { remountDemoIfNeeded(); syncEditorAppearance(); }).observe(root, { childList: true, subtree: true });

initWasm().then(() => {
  const initialLandingSource = initialTheme === "dark"
    ? landingSource
    : landingSource.replace('state theme = "dark"', `state theme = "${initialTheme}"`);
  const session = new DomSession(initialLandingSource, "Landing", "root");
  const err = session.rerender();
  if (err) console.error("Tint landing render error:", err);
}).catch((e) => console.error("Failed to render the landing page:", e));
root.addEventListener("click", (e) => {
  if (e.target.closest('[data-tag="GithubBtn"]') || e.target.closest('[data-tag="RepoLink"]')) window.open("https://github.com/tintlang/tint", "_blank", "noopener");
});
