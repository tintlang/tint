# Slot System

A type-safe, declarative way to pass UI content into a component: `HeaderSlot`,
`FooterSlot`, `IconSlot`, `ContentSlot`, `DefaultSlot`. Slots are typed, are not
functions, cannot be called, and are always declared inside a `ui fn`.

Unlike HTML/React, there's no `<slot/>` proxying, no named-children convention, no
shadow DOM — slots are static placeholders baked into the tree.

## Declaring slots

```
ui fn Card() {
    <Panel padding{20}>
        <HeaderSlot />
        <ContentSlot />
        <FooterSlot />
    </Panel>
}
```

Each slot is a placeholder the caller's content will replace. A slot can only be a
bare UI node — `<HeaderSlot />` or the equivalent `<HeaderSlot></HeaderSlot>`; a bare
`HeaderSlot` (no tag) or `<HeaderSlot>text</HeaderSlot>` (local content inside the
declaration) are both errors.

## Filling slots at the call site

```
<Card>
    <HeaderSlot>
        <Text>"Profile"</Text>
    </HeaderSlot>

    <ContentSlot>
        <Image source=avatar />
    </ContentSlot>

    <FooterSlot>
        <Button onClick=save>"Save"</Button>
    </FooterSlot>
</Card>
```

Calling `<Card>` builds the UI; the `<HeaderSlot/>` and `<ContentSlot/>` placeholders
it declares get replaced by the matching content passed in.

### `DefaultSlot` / `<Children/>`

If a component uses `<Children/>`, that's the default slot:

```
ui fn PanelBox() {
    <Panel radius{12px}>
        <Children />
    </Panel>
}

<PanelBox>
    <Text>"Inside"</Text>
    <Button>OK</Button>
</PanelBox>
```

All the children passed to `<PanelBox>` fill `<Children/>`.

## Rules

- Slots can only be declared inside a `ui fn` — never at file top level (`HeaderSlot`
  alone is an error).
- A slot can't carry logic or modifiers: `<HeaderSlot if=cond>` and
  `<HeaderSlot padding{20}>` are both errors — a slot is only a marker.
- A slot name must be unique within a component — declaring `<HeaderSlot/>` twice in
  the same `ui fn` is an error.
- A slot can't sit inside a `<Block>` — `<Block if=cond><HeaderSlot/></Block>` is an
  error.
- Slots can't nest inside each other: `<HeaderSlot><ContentSlot/></HeaderSlot>` is
  an error.
- A slot declaration can't have children of its own — only the caller supplies
  content: `<HeaderSlot><Text>"x"</Text></HeaderSlot>` inside the component's own
  definition is an error.

## Matching rules at the call site

- The slot name at the call site must match the declared slot exactly —
  `<Card><Header>...</Header></Card>` is an error if the component declared
  `<HeaderSlot/>`, not `<Header/>`.
- Slot order doesn't matter — passing `<HeaderSlot>` before or after
  `<ContentSlot>` produces the same result.
- Optional slots may simply be omitted; an unfilled slot resolves to an empty
  `UIChild`.
- Passing a slot the component never declared is an error
  (`<Card><UnknownSlot>...</UnknownSlot></Card>`).

## Nested components

Slot trees can go multiple levels deep — each `ui fn` has its own slot namespace,
and nested components' slots never collide with each other:

```
<Tabs>
    <Tab title="A">
        <Panel>
            <Children/>
        </Panel>
    </Tab>

    <Tab title="B">
        <Text>"Hello"</Text>
    </Tab>
</Tabs>
```

## Typing

A slot has type `SlotRef`: it isn't a variable, can't be compared, can't be passed
as a function argument, and only exists inside the UI tree.

```
HeaderSlot : SlotRef
FooterSlot : SlotRef
Children   : SlotRef
```

## Errors, collected

```
fn test() { <HeaderSlot/> }              // UI inside Logic Mode
<HeaderSlot/> <HeaderSlot/>                // duplicate slot
<Panel header=HeaderSlot/>                   // slot used as an attribute value
let x = HeaderSlot                            // slot used in an expression
<Card><header>...</header></Card>              // name doesn't match declaration
ui fn Card() { <HeaderSlot><Text>"x"</Text></HeaderSlot> }  // slot has its own children
```

## How slot resolution works, step by step

1. When a component is invoked, all of its children are collected.
2. Each child node is matched to a named slot.
3. `DefaultSlot` (`<Children/>`) collects everything that isn't matched to a named slot.
4. Inside the component, each slot placeholder is replaced with the matched content.
5. Any slot with no matching content becomes an empty `UIChild`.
6. The UI is built from the fully assembled tree.

## Full example

```
ui fn Card() {
    <Panel radius{12px} padding{20}>
        <HeaderSlot />
        <ContentSlot />
        <FooterSlot />
    </Panel>
}

ui fn App() {
    <Card>
        <HeaderSlot>
            <Text style=text{xl, bold}>
                "Dashboard"
            </Text>
        </HeaderSlot>

        <ContentSlot>
            <Panel padding{12}>
                <Text>"Hello World"</Text>
            </Panel>
        </ContentSlot>

        <FooterSlot>
            <Button onClick=logout>"Logout"</Button>
        </FooterSlot>
    </Card>
}
```
