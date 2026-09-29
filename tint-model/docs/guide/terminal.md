# Terminal programs

Tint can run logic programs directly in a terminal through the CLI.

## Development command

From the repository root, expose the development wrapper once:

```bash
export PATH="$PWD/scripts:$PATH"
```

Then use the normal CLI command:

```bash
tint run main.tn
```

The wrapper delegates to Cargo, so changes to the parser, evaluator, runtime,
or CLI are rebuilt automatically. There is no stale globally installed CLI to
refresh while developing Tint.

## Terminal I/O

The CLI host currently provides these native functions:

- `print(value)` writes a value without a newline;
- `println(value)` writes a value followed by a newline;
- `read_line()` reads one line from standard input;
- `parse_number(text)` returns `Result<number, string>`;
- `read_key()` reads one key event in terminal raw mode.

`read_key()` returns a string representation. Printable Unicode characters are
returned directly. Control and navigation keys use stable names such as
`Escape`, `Enter`, `ArrowUp`, and `Ctrl+c`.

`Result` values can be handled with `expect(message)` or `unwrap()`. `expect`
uses the supplied message when the value is `Err`, while `unwrap` requires no
arguments. Both methods return the payload from `Ok`. `Option` and `Result`
also provide `is_some`/`is_none`, `is_ok`/`is_err`, `unwrap_or`, `map`, and
`and_then` at runtime; their callback and payload types are checked
statically. The checker knows more methods (see `guide/types.md`), but the
runtime does not run them yet.

## Loops

Logic programs support `while` and `loop` statements, together with `break`
and `continue`. Functions containing interpreter control flow are routed
through the tree-walking runtime, which preserves terminal input and control
flow semantics.

## Release workflow

Users should install a released `tint` binary and do not need Rust or Cargo.
When the language or CLI changes, maintainers should:

1. update `VERSION`;
2. run `node scripts/sync-version.mjs`;
3. run the workspace checks from `CONTRIBUTING.md`;
4. publish a new CLI binary.

The development wrapper is for the repository checkout; it is not part of the
end-user installation.
