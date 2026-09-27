# Assets

UI nodes can declare a browser asset URL with `asset||`:

```tn
Image {
    asset||"/assets/pacman/pacman.png"
    width::128
    height::128
}
```

The current DOM backend renders `Image` with an `asset||` value as an `<img
src="...">`. Tint deliberately treats the value as a host-resolved URL for
now: the compiler does not read the filesystem or copy files. A project should
keep its distributable files under the web app's public asset directory and
ship a license/credits file next to third-party content.

Sprite-sheet slicing, preload handles, audio resources, and typed `Image`/
`Texture` values are the next resource-layer milestones; the current stable
surface is URL-backed image rendering.
