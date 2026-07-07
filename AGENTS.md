# AGENTS.md

Rust implementation for a unified live-grep file search (`srchr`). The binary
embeds file walking, content search, fuzzy-style TUI selection, syntax preview,
and editor launch behavior. Runtime external dependency: `$EDITOR` only.

## Structure

- `rust/` — Cargo crate for the `srchr` binary
- `rust/src/search.rs` — gitignore-aware name/content search and aggregation
- `rust/src/preview.rs` — preview windowing, binary/unreadable placeholders, syntect styling
- `rust/src/editor.rs` — `$EDITOR` resolution, path safety, launch args
- `rust/src/app.rs` — TUI state machine
- `rust/src/ui.rs` — ratatui rendering
- `rust/src/main.rs` — terminal setup, event loop, debounce, editor handoff
- `rust/tests/` — integration tests
- `docs/superpowers/specs/` — design docs; update when behavior changes
- `docs/superpowers/plans/` — implementation plans

## Behavior Invariants

- The tool must not require `fd`, `rg`, `fzf`, or `bat` at runtime.
- `$EDITOR` is required. If unset or empty, exit with a clear error instead of
  guessing an editor.
- Content and filename queries use smart-case regex semantics.
- Results are file-level rows: content hits show a match count; name-only hits
  show `[name]`.
- Selected paths may begin with `+` or `-`. Keep the guard that rewrites those
  relative paths to `./...` before calling `$EDITOR`; otherwise vim/nvim can
  treat `+...` as editor commands or `-...` as options.
- The interactive TUI needs a TTY. Agents cannot fully test it; ask the user for
  a manual smoke test after changes that affect interaction.

## Verification

Run the Rust checks from the workspace root:

```sh
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

GitHub Actions runs the same fmt/clippy/test gates in `.github/workflows/rust.yml`.

Manual TUI smoke test:

```sh
cargo run --manifest-path rust/Cargo.toml -- .
```

Check live typing, result counts, `[name]` rows, preview highlight, arrow-key
selection, `Enter` opening `$EDITOR`, and `Esc` restoring the terminal.
