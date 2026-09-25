# RuneEvents — UI Event System

A formal, type-safe event model. Runs inside the WASM/WebGPU runtime — there is no
DOM and no HTML-style bubbling underneath it.

## Two kinds of event construct

1. **Logic events**, via `=` — call a function:
   `onClick=submit`, `onKeyDown=handleKey`
2. **Animation events**, via `{}` — start an animation:
   `onHover{scale: 1 -> 1.1}`, `onBlur{opacity: 1 -> 0.8}`

Rule: Logic Mode always uses `=`; UI modifiers always use `{}`. Never mixed:
`onClick{...}` and `onHover=fn` are both errors.

All events run in Logic Mode except the animations they trigger. UI Mode itself
cannot contain functions, logic, `try`, or `await`; event payloads are only visible
to Logic Mode functions.

## Full event catalogue

- **Pointer:** `onClick onPress onRelease onHover onLeave onMove onWheel`
- **Gesture:** `onTap onLongPress onPan onPinch onRotate`
- **Keyboard:** `onKeyDown onKeyUp onKeyPress`
- **Focus:** `onFocus onBlur`
- **Scroll:** `onScroll`
- **Lifecycle:** `onMount onUpdate onDestroy`
- **Error:** `onError=handler` (fired on a failed async/`throws` operation)

## Payload types

```
struct ClickEvent    { x: f32, y: f32, button: i32 }
struct KeyEvent       { key: string, code: string, ctrl: bool, shift: bool }
struct ScrollEvent    { deltaX: f32, deltaY: f32 }
struct PanEvent       { deltaX: f32, deltaY: f32 }
struct PinchEvent     { scale: f32 }
struct RotateEvent    { angle: f32 }
struct FocusEvent     { focused: bool }
struct LifecycleEvent { timestamp: f64 }
```

## Logic handlers

The value is always a function name; the function takes zero or one argument (the
payload):

```
<Button onClick=increment />
fn increment() { count += 1; }

<Button onClick=handleClick />
fn handleClick(e: ClickEvent) {
    log{"Clicked at: {e.x}, {e.y}"}
}
```

## Animation-event modifiers

```
onHover{scale: 1 -> 1.1}
onLeave{scale: 1.1 -> 1}
```

Always use `{}`, describe UI animation only — no logic, no `fn` inside the block:
`onHover={fn { x = x + 1 }}` and `onPress={count += 1}` are both errors.

## Combining events on one component

```
<Panel
    onHover={opacity: 0.8 -> 1}
    onClick=openPopup
    onPress={scale: 1 -> 0.95}
    onBlur={opacity: 1 -> 0.6}
>
```

## Gestures

```
<Canvas onPan=moveCamera onPinch=zoom onRotate=rotateObject />

fn moveCamera(e: PanEvent) { camera.x += e.deltaX; camera.y += e.deltaY; }
fn zoom(e: PinchEvent)     { camera.scale *= e.scale; }
fn rotateObject(e: RotateEvent) { object.rotation += e.angle; }
```

Gestures are always Logic Mode (`=`); `onPan{scale:1->1.1}` is an error.

## Keyboard, focus, scroll, lifecycle, error

```
<TextField onKeyDown=handleKey />
fn handleKey(e: KeyEvent) { if e.key == "Enter" { submit(); } }

<TextField
    onFocus={border: gray-300 -> blue-500}
    onBlur={border: blue-500 -> gray-300}
>

<ScrollView onScroll=handleScroll />
fn handleScroll(e: ScrollEvent) { position += e.deltaY; }

<Panel onMount=init onDestroy=cleanup />
fn init(e: LifecycleEvent) { loadData(); }
fn cleanup() { dispose(); }

<Loader onError=handleError />
fn handleError(e: LoadError) { log{"Loading failed: {e.message}"} }
```

## Grammar

```
event_attribute = "on" identifier ( "=" logic_handler | modifier_block )
logic_handler   = identifier
modifier_block  = "{" animation_rule ("," animation_rule)* "}"
animation_rule  = property ":" value "->" value
identifier      = ascii_alpha (ascii_alnum)*
```

## Execution order

1. Pointer (`onPress` → `onClick` → `onRelease` → `onHover`/`onLeave`)
2. Gesture (pan, pinch, rotate)
3. Keyboard
4. Focus
5. Scroll
6. Lifecycle
7. Animation dispatch

## Logic (`=`) vs animation (`{}`) compatibility

| Event | Logic (`=`) | Animation (`{}`) |
|---|---|---|
| onClick | yes | no |
| onPress | yes | yes |
| onHover | no | yes |
| onLeave | no | yes |
| onPan / onPinch / onRotate | yes | no |
| onKeyDown | yes | no |
| onFocus / onBlur | no | yes |
| onScroll | yes | no |
| onMount / onDestroy | yes | no |
| onError | yes | no |

## Full example

```
ui App {
    state count = 0;

    <Column padding{24} spacing{20}>
        <Button
            onClick=increment
            onHover={scale: 1 -> 1.1}
            onLeave={scale: 1.1 -> 1}
        >
            "+"
        </Button>

        <Panel
            onPan=move
            onPinch=zoomCanvas
            padding{20}
            radius{12px}
            color{gray-800{20%}}
        >
            <Text>"Count: {count}"</Text>
        </Panel>
    </Column>
}

fn increment() { count += 1; }
fn move(e: PanEvent) { camera.x += e.deltaX; camera.y += e.deltaY; }
fn zoomCanvas(e: PinchEvent) { camera.zoom *= e.scale; }
```
