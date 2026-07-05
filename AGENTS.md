# AGENTS.md

Shell functions for a unified fd + rg + fzf + bat file search (`srchr <term>`).
No build system. Local smoke tests live in `tests/smoke.sh`; GitHub Actions runs the same script.

## Structure

- `srchr.fish` — fish implementation
- `srchr.sh` — bash **and** zsh implementation (one sourceable file; syntax is kept POSIX-compatible for both)
- `docs/superpowers/specs/` — design docs; update when behavior changes

## Critical invariant: keep the ports in sync

The two files implement the same function. The `sh -c` preview/enter snippets
passed to fzf must stay **byte-identical** between `srchr.fish` and `srchr.sh`.
Any behavior change goes into both files.

Quoting differs by necessity, not choice:
- fish: snippets are inline, using `\'` to escape single quotes (valid in fish only)
- bash/zsh: snippets are assembled from local vars (`locate`/`preview`/`open`)
  because those shells cannot escape `'` inside single quotes

The search term is never interpolated into the fzf command strings (injection
safety). It reaches the snippets via the `SRCHR_TERM` env var: `set -lx` in
fish, env prefix on the fzf call only (`| SRCHR_TERM=$term fzf`) in sh —
do not `export` it into the session.

Snippets run via `sh -c '...' sh {}` (file arrives as `$1`) so they work no
matter which shell fzf's `$SHELL -c` uses.

Selected paths may begin with `+` or `-` (raw output from `fd`/`rg`). Keep the
snippet guard that rewrites those relative paths to `./...` before calling
`rg`, `bat`, or `$EDITOR`; otherwise nvim/vim can treat `+...` as editor
commands and tools can treat `-...` as options.

## Verification

- Run the automated smoke suite: `tests/smoke.sh`
- The script covers syntax checks, fzf preview/bind parity across shell ports,
  direct snippet behavior, and security smoke checks for leading-option terms
  and selected paths beginning with `+` or `-`.
- zsh is optional locally: the script uses direct `zsh` when installed, falls
  back to Docker when available, and skips only when neither exists.
- The interactive fzf flow needs a TTY; the agent cannot test it — ask the
  user for a manual smoke test.
