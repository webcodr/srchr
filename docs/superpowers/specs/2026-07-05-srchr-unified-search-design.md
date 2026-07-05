# srchr: Unified fd + rg Search Design

**Date:** 2026-07-05
**Status:** Approved

## Purpose

Replace the two near-identical fish functions `fdp` (filename search via
`fd`) and `rgp` (content search via `rg`) with a single `srchr` function
that searches both filenames and file contents, presents combined results
in fzf, previews with bat, and opens the selection in `$EDITOR`.

## Requirements

- `srchr <search_term>`; empty term prints usage and returns 1.
- Candidate files come from both `fd -tf <term>` (filename matches) and
  `rg -lS <term>` (content matches, smart-case), merged and deduplicated.
- Smart preview: if the selected file contains the term, bat highlights
  the first matching line and starts the view 3 lines above it;
  otherwise plain bat from the top.
- Enter opens `$EDITOR +<line> file` for content matches (vim/nvim/helix
  style `+N`), plain `$EDITOR file` otherwise.
- `fdp.fish` and `rgp.fish` are deleted.

## Design

Single self-contained `srchr.fish` (approach chosen over a separate
helper-script variant to keep the tool a one-file install).

Key mechanics:

- `begin; fd -tf $term; rg -lS $term; end | sort -u` builds the
  deduplicated candidate list.
- The term is exported as `SRCHR_TERM` (`set -lx`) so fzf's preview and
  enter subprocesses can re-locate the first match via
  `rg -nS -m1 -- "$SRCHR_TERM" file`. This avoids interpolating the term
  into the command strings (injection-safe, no quoting of user input).
- Preview and enter logic are wrapped in `sh -c '...'` so they behave
  identically regardless of which shell fzf uses for `$SHELL -c`; fzf's
  `{}` placeholder is passed as `$1`.
- Enter uses fzf's `become(...)` to exec the editor in place.

## Rejected Alternatives

- **Helper preview script:** cleaner quoting but two files to keep in
  sync and place on `$PATH`.
- **Line-based `file:line:text` list (classic rg+fzf):** richer picker,
  but deduping filename hits against content hits gets ugly and the list
  format becomes inconsistent.

## Testing

- `fish -n srchr.fish` syntax check.
- Pipeline smoke test (`fd` + `rg` + `sort -u`) against the repo.
- Both `sh -c` snippets exercised directly with match/no-match terms and
  `EDITOR=echo` to verify arguments (`+1 file` vs `file`).
- Interactive fzf session verified manually (requires TTY).
