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
- Search terms are passed after `--` to prevent option injection into
  `fd` or `rg`.
- Smart preview: if the selected file contains the term, bat highlights
  the first matching line and starts the view 3 lines above it;
  otherwise plain bat from the top.
- Enter opens `$EDITOR +<line> file` for content matches (vim/nvim/helix
  style `+N`), plain `$EDITOR file` otherwise.
- Selected relative paths beginning with `+` or `-` are normalized with a
  `./` prefix before `rg`, `bat`, or `$EDITOR` sees them, so filenames
  cannot become editor commands or command-line options.
- `fdp.fish` and `rgp.fish` are deleted.

## Design

Single self-contained `srchr.fish` (approach chosen over a separate
helper-script variant to keep the tool a one-file install).

Key mechanics:

- `begin; fd -tf -- $term; rg -lS -- $term; end | sort -u` builds the
  deduplicated candidate list.
- The term is exported as `SRCHR_TERM` (`set -lx`) so fzf's preview and
  enter subprocesses can re-locate the first match via
  `rg -nS -m1 -- "$SRCHR_TERM" file`. This avoids interpolating the term
  into the command strings (injection-safe, no quoting of user input).
- Preview and enter logic are wrapped in `sh -c '...'` so they behave
  identically regardless of which shell fzf uses for `$SHELL -c`; fzf's
  `{}` placeholder is passed as `$1`.
- Enter uses fzf's `become(...)` to exec the editor in place.

## Shell Ports

`srchr.sh` provides the same function for bash and zsh in a single
sourceable file (the required syntax is identical in both shells).
Differences from the fish version are mechanical:

- The `sh -c` snippets are byte-identical, but built from local
  variables (`locate`, `preview`, `open`) sharing the rg-lookup
  fragment, since bash/zsh cannot escape `'` inside single quotes.
- `SRCHR_TERM` is passed as an env prefix on the fzf command only
  (`... | SRCHR_TERM=$search_term fzf ...`), not exported into the
  session.

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
- Shell ports: `bash -n`/`zsh -n` syntax checks (zsh via Docker),
  usage path in both shells, and fzf stubbed in both shells to confirm
  byte-identical `--preview`/`--bind` arguments and that `SRCHR_TERM`
  reaches fzf without leaking into the session.
- Security regression checks: leading-dash search terms do not become
  `fd`/`rg` options, and leading-plus/leading-dash selected paths are
  normalized before editor/preview invocation.
