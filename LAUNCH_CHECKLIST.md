# Reddit Launch Checklist

## Before Post (Day -1)

- [ ] Replace VSCode extension icon with proper rune-logo.svg
- [ ] Clean node_modules from vscode-rune/ (or add to .gitignore)
- [ ] Update runelang/README.md with:
  - CLI installation instructions
  - VSCode extension installation link
  - Quick start example
  - Link to sandbox demo
- [ ] Test CLI one more time:
  ```bash
  cargo build -p rune-cli --release
  ./target/release/rune build examples/app.rn -o /tmp/test.html
  ```
- [ ] Verify VSCode extension compiles:
  ```bash
  cd vscode-rune && npm run compile
  ```

## Reddit Post

- Title: "Rune — A UI Programming Language (Proof of Concept)"
- Use REDDIT_POST.md as base
- Post to r/rust
- Link to GitHub repo
- Mention: CLI available via cargo, VSCode extension, web sandbox

## Post-Launch (Week 2+)

- Publish CLI to crates.io
- Publish VSCode extension to marketplace
- Add issue templates (good first issue, etc.)
- Consider writing a blog post explaining the compiler architecture

## If Traction

- Reactive state system
- Click handlers that work
- Cross-file imports
- Better error messages
