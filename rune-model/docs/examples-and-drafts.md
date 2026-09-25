# Examples & Drafts

This file collects informal syntax experiments, scratch examples, and early drafts
that don't belong in the settled reference or guide docs — either because they
predate the current syntax, explore an alternative that was never finalized, or are
just quick sketches. Keep them for context; don't treat them as spec. Where a
snippet uses older syntax (like `::` instead of `{}` for modifiers), it's left as
originally written rather than "corrected."

## Quick token cheat-sheet

```
# Types
i32, f32, bool, string, unit

# Literals
123, -5, 3.14, "text", true, false, ()

# Arithmetic
+, -, *, /, %, unary -

# Comparison
==, !=, >, <, >=, <=

# Logical
&&, ||, !

# Assignment
=, +=, -=, *=, /=

# Access
obj.field
ns::item
arr[index]

# Function call
f(x)
max(a, b)

# Lambda
|x| expr
|a, b| expr

# Control flow
if / else
while
for x in xs
match

# Structs
StructName { field: value }

# Collections
[1, 2, 3]
{"a": 1, "b": 2}
```

## Expression grammar (early draft)

```
expression =
      literal
    | identifier
    | expression bin_op expression
    | unary_op expression
    | "(" expression ")"
    | call_expression
    | field_access
    | index_access
    | lambda_expression
    | match_expression

+ - * / %
> < >= <= == !=
&& || !
= += -=
```

## "Beast mode" — strict borrow example

An early illustration of the `[@(strict)]` attribute mentioned as a draft idea in
`guide/resources-and-borrowing.md`'s "Experimental" section — a mini-Rust mode with
borrow rules, lifetimes, and no GC:

```
fn [@(strict)] process<'a>(buf: &'a mut buffer) {
    borrow buf {
        fft(buf)
    }
}

[@(strict)]  // Beast Mode ON (no GC, borrow rules, lifetimes)

file {
    fn step<'a>(buf: &'a mut buffer) {
        borrow buf {
            normalize(buf)
            sobel(buf)
        }
    }
}
```

## Drivers, memory, and hardware access (exploratory)

A sketch of a lower-level, embedded/systems-programming surface — `driver`,
`irq`, `mmio`, `bitfield`, and `vault` blocks for hardware access. This is not part
of the documented language today; it's kept here because it shows a direction the
language might extend toward, beyond its GPU/UI focus.

```
driver Keyboard {
    irq 1
    buffer Size<256>
    uses io::Port(0x60)

    fn on_interrupt() {
        let sc = io::in8(0x60)
        buffer.push(sc)
    }
}

driver GPU {
    mmio Base(0xFED00000) {
        reg Command @ 0x04: u32
        reg Status  @ 0x08: u32
        reg Fence   @ 0x0C: u32
    }

    fn init() {
        mmio.Command.write(1)
    }
}

bitfield Status(u32) {
    busy: 0..1
    error: 1..2
}

if mmio.Status.read().busy == 1 {
    // gpu busy
}

driver Mouse {
    irq 12

    on start {
        io::out8(0x64, 0xA8)  // enable mouse
    }

    on stop {
        // disable mouse
    }

    fn on_interrupt() {
        let data = io::in8(0x60)
        process(data)
    }
}

let reg = mmio(0x4000_1000).as<u32>();
reg.write(0x1);

fn raw_test() {
    let buf = raw.alloc(64)
    raw.write(buf, 0, 42)
    let v = raw.read(buf, 0)
}
```

`vault` groups drivers and state together, as a kind of hardware-facing module:

```
vault Hardware {
    driver Keyboard {
        irq 1
        on start { io::out8(0x64, 0xAE) }         // enable keyboard
        fn on_interrupt() {
            let sc = io::in8(0x60)
            Buffer::push(keys, sc)
        }
    }

    driver Mouse {
        irq 12
        on start {
            io::out8(0x64, 0xA8)   // enable mouse
            io::out8(0x64, 0x20)   // request status
        }
        fn on_interrupt() {
            let d = io::in8(0x60)
            Buffer::push(mouse_stream, d)
        }
    }

    driver Timer {
        irq 0
        fn on_interrupt() {
            ticks += 1
            if ticks % 60 == 0 { callback::every_second() }
        }
    }

    state ticks = 0
    state keys = Buffer::new(128)
    state mouse_stream = Buffer::new(128)
}

vault Memory {
    const PAGE_SIZE = 4096
    state freelist = [ 0x100000, 0x104000, 0x108000 ]  // 3 free pages

    fn alloc_page() {
        let page = freelist.pop()
        zero(page)
        page
    }

    fn free_page(addr) { freelist.push(addr) }

    fn zero(addr) {
        let end = addr + PAGE_SIZE
        let mut p = addr
        while (p < end) {
            io::mem_write8(p, 0)
            p += 1
        }
    }

    fn write32(addr, value) { io::mem_write32(addr, value) }
    fn read32(addr) { io::mem_read32(addr) }
}

vault GPU {
    struct Command { opcode{u8}, a{u32}, b{u32}, c{u32} }

    state stream = Buffer::new(4096)

    fn push(cmd: Command) {
        raw.write32(stream, cmd.opcode)
        raw.write32(stream, cmd.a)
        raw.write32(stream, cmd.b)
        raw.write32(stream, cmd.c)
    }

    fn dispatch(kernel_id, x, y, z) {
        push(Command { opcode{1}, a{kernel_id}, b{x}, c{y << 16 | z} })
    }

    fn bind_texture(slot, tex_id) {
        push(Command { opcode{2}, a{slot}, b{tex_id}, c{0} })
    }

    fn submit() {
        io::gpu_submit(stream.raw_ptr())
        stream.clear()
    }
}

vault Sys {
    fn write(fd, buffer, len) { syscall(1, fd, buffer, len) }
    fn read(fd, buffer, len) { syscall(2, fd, buffer, len) }
    fn time() { syscall(13) }
    fn random(buf, size) { syscall(40, buf, size) }
    fn open(path, flags) { syscall(5, path.ptr(), flags) }
}

vault Shared {
    state buf = raw.alloc(256)

    // implicitly a strict borrow
    fn write(i, v) { raw.write(buf, i, v) }
    fn read(i) { raw.read(buf, i) }

    fn sum() {
        let mut s = 0
        let mut idx = 0
        while (idx < 256) {
            s += raw.read(buf, idx)
            idx += 1
        }
        s
    }
}
```

## A small todo app (Logic + UI)

An end-to-end sketch showing message-driven state updates paired with a UI layer.
Note this uses an earlier `ui ComponentName(...)` call-style syntax rather than the
`<Tag>` form documented in `ui/ui-syntax.md`.

```
struct Todo {
    id{i32}
    text{string}
    done{bool}
}

enum Msg {
    Add(string)
    Toggle(i32)
    Delete(i32)
}

fn update(list: [Todo], msg: Msg) -> [Todo] {
    match msg {
        Add(t) => {
            let id = list.length()
            return list.push(Todo { id{id}, text{t}, done{false} })
        }
        Toggle(id) => {
            return list.map(|item|
                if item.id == id { Todo { ..item, done{!item.done} } } else { item }
            )
        }
        Delete(id) => {
            return list.filter(|item| item.id != id)
        }
    }
}

ui TodoItem(item: Todo, onToggle: fn(i32), onDelete: fn(i32)) {
    Row {
        padding{10}
        spacing{12}
        align{center}

        Checkbox(checked: item.done, onClick: || onToggle(item.id))

        Text {
            text{item.text}
            size{lg}
            if item.done { color{gray-500} }
        }

        Spacer {}

        Button(onClick: || onDelete(item.id)) { "✕" }
    }
}

ui TodoList(list: [Todo], onMsg: fn(Msg)) {
    Column {
        spacing{16}

        Row {
            spacing{12}
            Input(value: "", onSubmit: |t| onMsg(Add(t))) { placeholder{"New task..."} }
            Button(onClick: || onMsg(Add("Quick Added"))) { "Add" }
        }

        Block {
            for item in list {
                TodoItem(item, |id| onMsg(Toggle(id)), |id| onMsg(Delete(id)))
            }
        }
    }
}

ui App() {
    let todos = [
        Todo { id{0}, text{"Buy milk"}, done{false} },
        Todo { id{1}, text{"Run Rune compiler"}, done{true} }
    ]

    Panel padding{20} radius{16px} color{gray-900{80%}} {
        Column {
            spacing{20}
            Text { text{"Rune Todo App"} size{2xl} weight{bold} }
            TodoList(todos, |msg| { todos = update(todos, msg) })
        }

        animate:onMount {
            opacity{0% -> 100%}
            y{20px -> 0px}
        }
    }
}
```

## Dashboard UI draft (`::` modifier syntax)

An earlier draft used `field::value` instead of `field{value}` for modifiers.
Kept verbatim as a syntax-history reference — do not use this form in new code; the
current form is documented in `ui/modifiers.md`.

```
<App theme::dark dpi::2.0>
    <Window title::"Control Panel" width::960 height::640 background::panel_bg>
        <Header height::64 padding::{left::24, right::24}>
            <Row gap::16 align::center>
                <Icon glyph::settings size::28 opacity::{0.3 -> 1.0} />
                <TextBlock text::"Dashboard" font::{size::24, weight::600} />
                <Spacer />
                <Button
                    text::"Refresh"
                    click::refresh_data
                    padding::{left::14, right::14, top::6, bottom::6}
                    animate::{opacity::{0.0 -> 1.0}}
                />
            </Row>
        </Header>

        <Row gap::20 padding::{left::24, right::24, top::20} height::100%>
            <Sidebar width::220 radius::8 background::panel_bg>
                <ListView gap::12 padding::{top::16}>
                    <SidebarItem glyph::home   label::"Home"     active::true  />
                    <SidebarItem glyph::chart  label::"Stats"    active::false />
                    <SidebarItem glyph::user   label::"Profile"  active::false />
                    <SidebarItem glyph::config label::"Settings" active::false />
                </ListView>
            </Sidebar>

            <Column gap::24 width::100%>
                <Card radius::12 padding::{all::20} background::card_bg>
                    <TextBlock text::"Server Metrics" font::{size::20, weight::600} />
                    <Row gap::16>
                        <StatBox label::"CPU Load" value::{cpu_load} icon::{glyph::cpu, size::20} trend::{value::cpu_trend, color::green} />
                        <StatBox label::"Memory" value::{mem_usage} icon::{glyph::ram, size::20} trend::{value::mem_trend, color::blue} />
                        <StatBox label::"Network" value::{net_usage} icon::{glyph::wifi, size::20} trend::{value::net_trend, color::orange} />
                    </Row>
                </Card>

                <Card radius::12 padding::{all::20}>
                    <Row justify::space-between align::center>
                        <TextBlock text::"Traffic Graph" font::{size::18, weight::600} />
                        <Button text::"Export" click::export_data padding::{left::10, right::10} />
                    </Row>
                    <Graph height::280 data::{traffic_data} animate::{opacity::{0.0 -> 1.0}, scale::{0.9 -> 1.0}} />
                </Card>

                <Card radius::12 padding::{all::20}>
                    <Row gap::20 align::center>
                        <Avatar glyph::user size::48 />
                        <Column gap::4>
                            <TextBlock text::user.name font::{size::18} />
                            <TextBlock text::user.role opacity::0.65 />
                        </Column>
                        <Spacer />
                        <Button text::"Logout" click::logout padding::{left::12, right::12} />
                    </Row>
                </Card>
            </Column>
        </Row>

        <Footer height::36 padding::{left::24, right::24}>
            <TextBlock opacity::0.6>"Last sync: {last_sync_time}"</TextBlock>
        </Footer>
    </Window>
</App>
```

## Assorted generics / matching / GPU test cases

Small standalone cases used while stress-testing the parser (generics, deep
matching, GPU kernel calls, borrow, default params) — kept for reference, several
already promoted into `project-status.md`'s "Implemented" list:

```
struct Pair<T, U> { a{T}, b{U} }

fn AdvancedMonster(x, y {10}) {
    let a = (1, (2, (3, (4, [x, y, x+y]))))
    let b = match a {
        (x, (y, (z, (k, arr)))) => {
            let f = |u| u * 2
            f(x + y + z + k + arr[2])
        }
    }
    let c = borrow(x) { borrow@group(y) { x + y + b } }
    let d = Logic::deep_mix(x, y)
    (b + c + d) * 2
}

kernel invert(tex: Texture) -> Texture {
    let id = global_id()
    let px = tex.read(id)
    write(px)
}

fn TestGpuCall(frame: Texture) -> Texture {
    invert.gpu(frame)
}

fn def_add(a, b{10}) { a + b }
fn TestDefaults() { def_add(5) + def_add(5, 20) }   // 5+10 + 5+20 = 40

fn BorrowTest(x) {
    borrow(x) { x + 10 }
}
```
