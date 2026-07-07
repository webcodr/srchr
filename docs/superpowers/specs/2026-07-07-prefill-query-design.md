# Prefill Query Parameter — Design

## Summary

Add a command-line flag that prefills the interactive search query so `srchr`
opens with a query already entered and results already displayed. This turns
`srchr` into a usable launch target for shell aliases and editor integrations
that want to jump straight into a search.

## Motivation

Today the query always starts empty (`app.rs`), and argument parsing is a single
`std::env::args().nth(1)` call that treats the first positional argument as the
search root (`main.rs`). There is no way to open the tool with a query already
populated. A prefill flag lets callers seed the search without simulating
keystrokes.

## CLI

Argument parsing moves to [`clap`](https://docs.rs/clap) (derive API).

- New dependency in `rust/Cargo.toml`:
  `clap = { version = "4", features = ["derive"] }`.
- A `Cli` struct defined in `main.rs`:
  - `path: PathBuf` — positional, defaults to `.`. Replaces the current
    `args().nth(1)` logic.
  - `query: Option<String>` — `-q` / `--query`, optional.
- Help text uses clap's minimal derive defaults: name and version inferred from
  `Cargo.toml`, plus a short `about` string. `--help`/`-h` and argument error
  messages come for free.

Examples:

```sh
srchr                 # search '.', empty query (unchanged behavior)
srchr src             # search 'src', empty query (unchanged behavior)
srchr -q TODO         # search '.', query prefilled with "TODO"
srchr src --query fn  # search 'src', query prefilled with "fn"
```

## Prefill Behavior

- `App` gains `App::with_query(String)`. `App::new()` delegates to
  `App::with_query(String::new())` so existing callers are unaffected.
- `run()` takes the resolved `path` and `Option<String>` query, and seeds the
  app via `App::with_query`.
- If the prefilled query is non-empty, the search runs immediately on startup:
  `run()` seeds `pending_query = Some(query)` with `pending_at` set far enough in
  the past that the debounce fires on the first loop iteration, and sets
  `status = "searching..."`. Results appear as soon as the TUI opens.
- The prefilled query is fully editable — backspace and further typing behave
  exactly as if the user had typed it.

## Edge Cases

- `-q ""` (explicit empty string) behaves identically to no `-q`: empty query,
  no startup search, empty status.
- Omitting `-q` is byte-for-byte the current behavior.

## Testing

- Unit test: `App::with_query("foo")` yields `query == "foo"`; `App::new()`
  yields an empty query.
- Clap sanity test: `Cli::command().debug_assert()` in a `#[test]` catches
  malformed argument definitions at test time.
- The startup-search wiring lives inside the TTY event loop and cannot be fully
  exercised headlessly; it is covered by the manual smoke test per `AGENTS.md`.
  The smoke test should confirm that `srchr -q <term>` opens with the query
  shown and results already populated, and that editing the prefilled query
  works.

## Non-Goals

- No change to search semantics, preview, or editor handoff.
- No reading the query from stdin, a file, or environment variables.
- No multi-query or history support.
