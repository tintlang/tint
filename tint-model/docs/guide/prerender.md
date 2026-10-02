# Prerendered pages

`tint build` writes the first screen of the app into the page as static HTML, so there is content
(and text for crawlers and link previews) before the WebAssembly has loaded. The runtime replaces it
with the live page in one step.

```
tint build site.tn -o dist/index.html
compiled: 212 KB of WebAssembly
prerendered: 18 KB of HTML
```

- The entry `ui fn` is run once at build time. If it cannot run there (it calls a browser function
  while starting, say) the build says `prerender skipped` and the page loads as before.
- Only the route `/` is prerendered; on another path the static content is dropped before it shows.
- Elements, text, inline styles and attributes are written. Hover, breakpoint and state styles,
  and everything that runs, wait for the runtime; a `view::` reveal is written in its final state.
