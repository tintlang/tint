# Tint

**One language for UI structure, style, state and logic.** Tint compiles to
Rust/WASM and renders into the real DOM, so you write small UIs in a single
`.tn` file instead of juggling HTML, CSS and JavaScript.

> **Status: experimental.** Tint is a personal project and the language is
> still changing. It is not production-ready. I'm still finding out where it
> fits best, and feedback on that is welcome.

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
in JS. Three languages, three files, and nothing checks that they agree.
Tint puts them in one statically typed language:

- **Structure and style together.** Nodes take `layout::`, `paint::` and
  `motion::` modifiers directly. Themes, design tokens, reusable styles,
  component variants and responsive breakpoints are part of the language.
- **State and logic next to the UI.** `state`, event handlers, `if`/`for`/`match`
  and typed functions are ordinary language features. The type checker sees
  the UI and the logic together.
- **One small thing to learn.** I think Tint is easier to learn than React
  together with CSS and a build setup. You learn one small language and use
  one tool (`tint dev` / `tint build`), with no Node, bundler or framework to
  set up.
- **No virtual DOM, no framework.** `DomSession` creates real DOM elements and
  updates them directly through `web-sys`.
- **Rust when you need it.** Heavy work stays in Rust and is called from Tint
  by name (see [Tint and Rust](#tint-and-rust)).

## You don't write HTML, CSS or JS

The idea is that you stop writing those files by hand. You write Tint, only
for the UI and its logic, and Tint does the rest: `tint dev` and `tint build`
generate the HTML page, the styles, fonts, animations and routes come from
`.tn`, and the runtime that touches the DOM is generated Rust/WASM.

The Tint site and sandbox are built this way: one program (`site.tn`) with no
hand-written HTML, JS or CSS files, including the syntax-highlighting editor
and the live preview. Generated output (the page shell and the wasm-bindgen
glue) still exists, but you never edit it. Tint packages for embedding in
existing apps (Vite, React, Svelte) and the VS Code extension are separate
and do use JS/TS.

## What it's for

Tint is meant for the UI layer of an application:

- **Small apps written entirely in Tint.** Tools, dashboards, games. The
  [Pong demo](https://tint-gamma.vercel.app/pong) is a real example: game
  state, collision logic and rendering are all `.tn` code.
- **The UI and UI logic of a bigger app.** Tabs, windows, popovers, overlays
  and their state. The backend, engine and server stay in Rust.

It is not a replacement for Rust (or any other language) on the backend, and
it does not try to replace HTML for content sites.

## How it fits together

- **Logic** is written in Tint (`state`, `fn`, event handlers). For heavy work
  or system access, register Rust functions as *natives*.
- **Rendering** is done by Tint. It owns the subtree it renders and writes
  real DOM elements. There is no canvas.
- **Embedding.** Mount a `.tn` file into any element of an existing page with
  `@tintlang/runtime`. Vite, React and Svelte packages are available, and
  `ref||` / `js||` let host JavaScript talk to Tint nodes
  ([escape hatches](tint-model/docs/guide/escape-hatches.md)).

```ts
import { mount } from "@tintlang/runtime";
await mount(source, document.querySelector("#app")!, { entry });
```

### Tint and Rust

```rust
vm.register_native("now_ms", |args| { /* ordinary Rust */ });
```

`.tn` code then calls `now_ms()` like any other function. There is no Rust
syntax inside the language.

## Try it

- **Live sandbox:** https://tint-gamma.vercel.app/
- **Pong, written in Tint:** https://tint-gamma.vercel.app/pong

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
