# srchr Rust port — design

**Date:** 2026-07-07
**Status:** Approved (design), pending implementation plan

## Goal

Replace the two shell implementations (`srchr.fish`, `srchr.sh`) with a single
self-contained Rust binary. The binary depends on **no external runtime tools
except the user's `$EDITOR`** — it embeds the roles currently played by `fd`,
`rg`, `fzf`, and `bat`.

The Rust binary is the successor. The shell ports remain only until the binary
reaches behavioral parity, then they are retired.

## Motivation

The current tool delegates to `fd`, `rg`, `fzf`, and `bat`. The primary driver
for this port is **eliminating those external dependencies** so the tool is a
single binary that works without the user installing four separate programs.
Dropping the dual shell-port maintenance burden (and the byte-identical fzf
snippet invariant) is a secondary benefit.

## Behavior (settled)

The port also evolves the interaction model from the shell version's
"term given once, fuzzy-filter over paths" into a **live-grep**:

- **Live-grep:** the typed query is a live content search; results refresh as
  the user types (debounced).
- **Rows = files with match counts.** Each result row is a file plus its match
  count. Name-only matches display `[name]` instead of a count.
- **Merged sources (name + content).** A file appears if its **name** matches
  OR its **content** matches (preserving today's `fd` + `rg -l` dual nature),
  deduped so a file matching both appears once.
- **Smart-case regex** for both content and filename matching: regex, and
  case-insensitive unless the query contains an uppercase character (matches
  `rg -S` / `fd` defaults).
- **Preview:** syntax-highlighted. For a content hit, highlight the **first**
  matching line with ~3 lines of context above. For a name-only hit, show the
  file from the top with no highlight.
- **Enter:** launch `$EDITOR` at the first match (`+<line> <path>`), or just the
  file (`<path>`) for name-only hits.
- **Path safety:** paths beginning with `+` or `-` are rewritten to `./…`
  before being handed to the editor (the AGENTS.md invariant).

## Architecture

Approach: a bespoke `ratatui` TUI over the ripgrep-family search crates and
`syntect` for preview. This is chosen over reusing the `skim` crate because the
UX (file rows with counts, live content search, custom preview highlighting)
does not map onto skim's fuzzy-over-static-list model.

### Project layout

```
srchr/
  srchr.fish, srchr.sh        # kept until parity, then retired
  rust/
    Cargo.toml
    src/
      main.rs        # arg parsing, terminal setup/teardown, top-level wiring
      search.rs      # walker + content search + filename match + aggregation
      app.rs         # TUI state machine (query, results, selection, preview)
      ui.rs          # ratatui rendering (input box, results list, preview pane)
      preview.rs     # syntect highlighting + line-range/highlight logic
      editor.rs      # $EDITOR launch + +/- path-safety guard
    tests/
      search.rs      # integration tests over a fixture tree
```

### Key crates

`ignore`, `grep-regex`, `grep-searcher`, `regex`, `ratatui`, `crossterm`,
`syntect`.

### Module boundaries

- `search` — `fn search(query, root, cancel_flag) -> Vec<FileHit>` where
  `FileHit { path, match_count, first_line: Option<usize> }`. No UI knowledge.
- `app` — holds state, reacts to events; owns no rendering or blocking I/O.
- `ui` — pure rendering from `app` state.
- `preview`, `editor` — pure functions of a path (+ optional line).

`search` and `preview` are unit-testable without a terminal. The interactive
TUI itself cannot be auto-tested (same caveat as the shell version).

## Search pipeline

Per query:

1. **Compile query once.** Detect smart-case (any uppercase → case-sensitive).
   Build a `grep-regex` matcher (content) and a `regex::Regex` (filename) from
   the same pattern and case flag. If the regex fails to compile (user
   mid-typing), treat as no results and show a subtle hint; recover when valid.
2. **Walk the tree** with `ignore::WalkBuilder` (gitignore + hidden-file rules,
   matching `fd`/`rg` defaults), parallel walk for speed.
3. **Per file → at most one `FileHit`:**
   - Filename match: path/file name matches the filename regex → contributes
     the file even with 0 content matches.
   - Content match: run `grep-searcher`; count matching lines, record the
     **first** matching line number (`-m1` equivalent for preview/jump; full
     count still displayed).
   - Name-only hit: `match_count = 0`, `first_line = None`.
   - Binary files: skipped for content (searcher detects), like `rg`.
4. **Aggregate & order:** dedupe by path. Order: content matches first
   (descending match count, then path), name-only matches last.
5. **Cancellation & debounce:** debounce ~50–80 ms per keystroke. Searches run
   on a background thread with an `AtomicBool` cancel flag; a newer query
   cancels the in-flight search. Results stream back via a channel; `app`
   replaces the list on completion so the UI never blocks.

## TUI layout & interaction

```
┌ query ────────────────────────────────┐
│ > search term▏                         │
├ results ──────────────┬ preview ───────┤
│ src/app.rs      (12)   │ (syntect-      │
│ src/search.rs    (7)   │  highlighted   │
│ README.md        (3)   │  preview of    │
│ Cargo.toml    [name]   │  selected file)│
│ ...                    │                │
└────────────────────────┴────────────────┘
 42 files · rg-regex · smart-case
```

- Top: input box (live query).
- Left: results list — path + right-aligned match count; name-only → `[name]`.
- Right: preview pane for the selected row.
- Bottom: status line (result count, mode hints).

**Keybindings:**
- Printable / Backspace → edit query (debounced search).
- `Up`/`Down` or `Ctrl-p`/`Ctrl-n` → move selection (updates preview).
- `Enter` → launch `$EDITOR` at selected hit, exit.
- `Esc` / `Ctrl-c` → quit, no action.

**Event loop:** `crossterm` event stream. Query change → debounce → background
search. Results arrive via channel → update list, clamp selection, refresh
preview. Selection move → recompute preview only (no re-search). Preview is
computed **only for the selected row** (lazily), not for every row.

**Deferred (YAGNI):** `PgUp`/`PgDn` preview scrolling.

## Preview rendering (bat replacement, via syntect)

- Bundled syntaxes + a fixed default theme (e.g. `base16-ocean.dark`). Syntax
  detected by extension, falling back to first-line/plain. These sets are
  embedded in the binary (the main binary-size cost — the same assets `bat`
  ships).
- **Content hit:** render `start = max(1, first_line - 3)` through the visible
  pane height, marking `first_line` with a distinct background. Mirrors
  `--highlight-line` + `--line-range "$start:"` with 3 lines of context above.
- **Name-only hit:** render from the top, no highlight (the `else bat …`
  branch).
- Convert syntect styled spans into ratatui `Line`/`Span` with matching colors.
- **Guardrails:** cap bytes/lines read to the visible window + margin; binary or
  unreadable files show a placeholder.
- **Known tradeoff:** syntect output will not be pixel-identical to the user's
  personal `bat` theme/config — inherent to dropping the `bat` dependency. A
  theme flag/config can come later.
- **Considered and deferred: honoring the user's bat theme.** bat is built on
  syntect and its themes are `.tmTheme` files syntect can load, so honoring
  `$BAT_THEME` / bat's config is technically possible. Deferred because
  syntect's built-in theme set only bundles a handful of themes; resolving an
  arbitrary bat theme name would require embedding bat's larger theme
  collection or loading `.tmTheme` files from bat's config dir at runtime —
  a soft coupling to bat's config not worth it for this iteration. Revisit via
  a `--theme` flag / optional `$BAT_THEME` lookup later.

## Editor launch & path safety

- **Resolution:** read `$EDITOR`; if unset (or empty), exit with a clear error
  ("$EDITOR is not set") rather than guessing an editor.
- **Path safety:** if a path begins with `+` or `-`, rewrite to `./<path>`
  before handing it to the editor (vim treats leading `+` as a command, leading
  `-` as an option). Also pass paths after `--` where supported.
- **Launch:**
  - Content hit → `EDITOR +<line> <path>`.
  - Name-only hit → `EDITOR <path>`.
- **Terminal handoff:** before spawning, fully restore the terminal (leave
  alternate screen, disable raw mode, show cursor), then run the editor as a
  foreground child inheriting our stdio so it owns the tty. On editor exit,
  srchr exits (equivalent to the shell version's `exec`; net user-visible
  behavior is identical — pick → editor opens → done).
- **`+<line>` portability:** the `+N` convention (vim/nvim/emacs/nano) is
  supported now. A per-editor mapping (e.g. VS Code `-g file:line`) is a later
  enhancement.

## Error handling & edge cases

- **Invalid/partial regex:** no crash — keep last good results or show empty
  list with an "invalid pattern" status hint; auto-recover when valid.
- **Empty query:** show nothing (no whole-tree match-everything walk).
- **No matches:** empty list + "no matches" status; Enter does nothing.
- **Unreadable / permission-denied:** skipped silently during the walk.
- **Binary files:** skipped for content; a selected name-only binary file shows
  a "binary file" placeholder.
- **Huge files:** preview reads only the needed window; search streams
  line-by-line.
- **Terminal too small:** degrade gracefully (collapse preview below a min
  width).
- **`$EDITOR` unset/empty:** exit with a clear error; no editor is guessed.
- **No TTY (piped/non-interactive):** detect and exit with a clear message.

## Testing & verification

- **`cargo test` integration tests** (`tests/search.rs`) over a fixture tree:
  - filename-only match appears with `[name]` / `first_line = None`
  - content match yields correct count + first line
  - file matching both name and content appears once
  - smart-case: lowercase query case-insensitive; uppercase makes it sensitive
  - gitignore respected
  - path-safety: `+`/`-` paths normalized to `./…`
- **Unit tests** for `preview` (line-range/highlight math) and `editor`
  (arg-vector construction incl. `+N` and `+`/`-` guard).
- **`cargo clippy` + `cargo fmt --check`** as the lint/syntax gate.
- **CI:** extend GitHub Actions with a Rust job (fmt/clippy/test).
- **Interactive TUI flow:** cannot be auto-tested — manual smoke test by the
  user (same caveat as today).

## Out of scope (for this iteration)

- Preview scrolling (`PgUp`/`PgDn`).
- Per-editor line-jump mappings beyond `+N`.
- User-configurable preview theme.
- Removal of the shell ports (happens after parity is confirmed).
