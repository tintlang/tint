# Tint

**One language for UI structure, style, state and logic.** Tint compiles to
Rust/WASM and renders into the real DOM, so you write small UIs in a single
`.tn` file instead of juggling HTML, CSS and JavaScript.

> **Status: experimental.** Tint is a personal project and the language is
> still changing. It is not production-ready, and feedback on where it fits
> best is welcome.

<p align="center"><img src="assets/header.png" alt="tint language" width="100%"></p>

A small example of the syntax, a counter:

```tint
ui fn Counter() {
    state count = 0

    Page {
        layout::{ direction::column, padding::24, gap::12 }
        Text { text::{32, bold} "{count}" }
        Button {
            click||increment
            layout::{ padding.x::18, padding.y::10 }
            paint::{ radius::full, background::#12141a, color::#ffffff }
            "Add one"
        }
    }
}

fn increment() {
    count = count + 1
}
```

Layout, paint, hover effects, state and the click handler live in one place,
with no separate stylesheet and no framework.

## Why Tint

In a web UI the structure is in HTML, the look is in CSS and the behaviour is
in JS: three languages, and nothing checks that they agree. Tint puts them in
one statically typed language:

- **Structure and style together.** Nodes take `layout::`, `paint::` and
  `motion::` modifiers directly. Themes, tokens, variants and responsive
  breakpoints are part of the language.
- **State and logic next to the UI.** `state`, event handlers, `if`/`for`/`match`
  and typed functions are ordinary language features, checked together with the UI.
- **No HTML, CSS or JS to write.** `tint dev` / `tint build` generate the page,
  and the DOM runtime is generated Rust/WASM. No Node, bundler or framework.
  The Tint site itself is one `.tn` program.
- **No virtual DOM.** `DomSession` creates real DOM elements and updates them
  directly through `web-sys`. There is no canvas.

## What it's for

The UI layer of an application: small apps written entirely in Tint (tools,
dashboards, games such as the [Pong demo](https://tint-gamma.vercel.app/pong)),
or the UI and its state inside a bigger app. Backend and engine stay in Rust
(or anything else). It does not replace HTML for content sites.

To use it in an existing page, mount a `.tn` file with `@tintlang/runtime`
(Vite, React and Svelte packages exist too):

```ts
import { mount } from "@tintlang/runtime";
await mount(source, document.querySelector("#app")!, { entry });
```

## Rust and JavaScript

```tint
app { rs::"./native.rs" js::"./utils.js" css::"./theme.css" }
```

Functions marked `#[tint::export]` in the Rust file and everything a JS module
exports are callable from `.tn` by name. `tint build` compiles the Rust to
WASM, so it works in the browser with no server. Later results (timers,
`fetch`, Rust `async fn`) arrive through a callback; `await` in an `async fn`
is sugar for it. See [Rust interop](tint-model/docs/guide/rust-interop.md),
[escape hatches](tint-model/docs/guide/escape-hatches.md) and
[async](tint-model/docs/guide/async.md).

## Try it

- **Live sandbox:** https://tint-gamma.vercel.app/

Run it locally:

```bash
# install the CLI (macOS/Linux)
curl -fsSL https://raw.githubusercontent.com/tintlang/tint/main/scripts/install-tint.sh | bash

tint check app.tn                # check syntax and types
tint run app.tn                  # run a program
tint dev app.tn                  # dev server with live reload, no HTML needed
tint build app.tn -o app.html    # standalone HTML with embedded WASM
```

Windows install, the VS Code extension and the web sandbox are covered in the
[docs](tint-model/docs/README.md). Using Tint in an existing Vite app:
[`vite.md`](tint-model/docs/guide/vite.md).

## Documentation

- [Feature list and current status](FEATURES.md)
- [Language and UI docs](tint-model/docs/README.md)
- [Contributing and development setup](CONTRIBUTING.md)

## License

MIT, see [LICENSE](LICENSE).

Made by Mark Bender ([@hawerz](https://github.com/hawerz)).
