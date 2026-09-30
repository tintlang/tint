# App metadata and routing

A Tint program can describe its own page, so no HTML file is needed.

```tn
app { title::"My app" lang::"en" router::on }

ui fn App() {
    Page {
        Nav {
            HomeLink { route||"/" "Home" }
            AboutLink { route||"/about" "About" }
        }
        Text { if{route_path == "/"} "Home page" }
        Text { if{route_path == "/about"} "About page" }
    }
}
```

`tint dev app.tn` serves it with live reload (every path returns the app, so
reloading `/about` works); `tint build app.tn -o app.html` writes one file.
See `examples/router.tn`.

## `app { ... }`

| key      | meaning                                                              |
|----------|----------------------------------------------------------------------|
| `title`  | the page `<title>` (`tint build`/`dev`, and set at runtime in a host page) |
| `lang`   | the `<html lang>` attribute                                          |
| `router` | `on` turns on the client router below                                |
| `route.Ui` | `route.Home::"/"` or `route.About::{ path::"/about", title::"About" }`: with `router::on`, the URL picks which `ui fn` renders. Each page is its own `ui fn` with its own `state` (`theme` carries over); a link `route||"/about"` switches without a page load |
| `page`   | `page::{ margin::0, background::#0a0a12 }` styles the host `<body>` (same syntax as `layout::{ }`; also `color-scheme`) |
| `keyframes.name` | `keyframes.spin::{ from::{ opacity::0 }, p50::{ opacity::0.5 }, to::{ opacity::1 } }` becomes `@keyframes spin`; `pN` is N%. Use with `animation::"spin 1s infinite"` |
| `font.Family` | `font.Inter::{ src::"/inter.woff2", weight::400 }` becomes `@font-face` (also `style`, `display`) |

## `route_path` and `router::on`

`route_path` is a string variable holding the current URL path (`"/"`,
`"/about"`). It is read from the URL on every render, so back/forward and typed
URLs just work.

With `router::on`, a link written `route||"/about"` navigates through the history
API and re-renders instead of loading a page. Links with `target||"_blank"`,
external URLs and modified clicks (Ctrl/Cmd/Shift) stay ordinary links. Without
`router::on`, links are plain links and nothing is intercepted.

A page opened from disk (`file://`) can't use the history API, so the router
uses `#/about` there.

## Width breakpoints

Besides `mobile`, `tablet`, `laptop` and `desktop`, a node accepts
`max-<px>::{ ... }` (viewport at most that wide) and `min-<px>::{ ... }`. Matching
blocks are layered over the base style in source order, like CSS media queries,
so write `max-*` blocks from the largest to the smallest:

```tn
Field {
    layout::{ width::480 }
    max-560::{ scale::0.72, transform-origin::"top center", margin.b::-72 }
    max-420::{ scale::0.62, margin.b::-97 }
}
```

## One program, many pages

`route.*` lets a single Tint program hold several pages instead of one HTML file
per page. `sandbox/src/site.tn` is the example: the landing page, Pong, Pac-Man
and the sandbox are four `ui fn`s in one source. Global `fn`s share one
namespace across the program, so give page-specific handlers distinct names
(`pac_restart`, not `restart`).
