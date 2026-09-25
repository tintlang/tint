# `<Block>` — the Logic Layer

`<Block>` is a logic container: it never renders anything, it just controls
structure inside the UI tree.

## If

```
<Block if=isVisible>
    <Text> "Visible" </Text>
</Block>
```

## Else

```
<Block if=ready>
    <Text>"OK"</Text>
</Block>
<Block else>
    <Text>"Loading..."</Text>
</Block>
```

Only one `<Else>` is allowed, and only right after a `<Block if>`.

## For

```
<Block for{item in items}>
    <Panel> "{item.name}" </Panel>
</Block>
```

## Match

```
<Block match=status>
    <case ready> <Text>"Ready"</Text> </case>
    <case error> <Text>"Error"</Text> </case>
</Block>
```

Conditions can be arbitrary logic expressions:

```
<Block if{x + y > deep_mix(a, logic.sum())}>
```

## Notes

`<Block>` creates no visual elements, may contain nested `<Panel>` and other UI
nodes, can nest inside itself, and is the natural tool for dynamic UI:

```
<Block if=user != null>
    <Panel> "{user.name}" </Panel>
</Block>
```

`<Block>` is control flow for the UI tree — see `03-language-reference.md` §4 for
the Logic-Mode equivalents (`if`, `for`, `match`) it mirrors.
