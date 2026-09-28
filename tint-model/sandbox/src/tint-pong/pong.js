import "./pong.css";
import initWasm, { DomSession } from "../../pkg-web/tint_wasm.js";
import pongSource from "./pong.tn";

const root = document.getElementById("root");

// Keyboard input and the game clock are both declared in pong.tn now
// (`key_down||on_key_down`, `frame||on_frame` on PongGame's Page) and
// handled entirely by tint-wasm's DOM renderer -- see
// crates/tint-wasm/src/dom/{render.rs,helpers.rs}. This file is just the
// host bootstrap: load the wasm module and mount the session.
initWasm().then(() => {
  const session = new DomSession(pongSource, "PongGame", "root");
  const renderError = session.rerender();
  if (renderError) throw new Error(renderError);
}).catch((error) => {
  console.error("Failed to start Tint Pong:", error);
  root.textContent = `Tint Pong failed to start: ${error?.message ?? error}`;
});
