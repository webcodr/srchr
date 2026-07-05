# srchr: Local and GitHub Test Design

**Date:** 2026-07-05
**Status:** Approved

## Purpose

Add lightweight automated regression coverage for `srchr` that can be run
locally and on GitHub. The tests should automate the existing manual
verification checklist where practical without introducing a larger test
framework than this small shell-function repository needs.

## Scope

- Add one local smoke-test script.
- Add one GitHub Actions workflow that runs the same script.
- Keep interactive fzf behavior as a manual smoke test because it requires a
  real TTY.
- Do not change `srchr.fish` or `srchr.sh` behavior as part of this work unless
  a test exposes an existing bug.

## Design

Create `tests/smoke.sh` as the single local entry point for automated checks.
The script will be plain POSIX-compatible shell where practical, with small
temporary fixtures and command stubs as needed.

The script will cover:

- Syntax checks: `fish -n srchr.fish`, `bash -n srchr.sh`, and `zsh -n srchr.sh`.
  Locally, the script should use `zsh` when installed, otherwise try the
  documented Docker fallback, and skip only when neither is available.
- Preview and enter snippets: run the `sh -c` snippets directly with controlled
  `SRCHR_TERM`, stubbed `rg`, stubbed `bat`, and `EDITOR=echo`-style behavior
  to verify match and no-match paths.
- Port parity: source or load both shell implementations with a stubbed `fzf`,
  capture the `--preview` and `--bind` arguments, and confirm the effective
  preview and enter snippets are byte-identical between fish and sh.
- Security smoke checks: use `fd`/`rg` stubs to verify leading-option search
  terms are passed after `--`, and selected relative paths beginning with `+` or
  `-` are normalized to `./...` before reaching `rg`, `bat`, or `$EDITOR`.

Add `.github/workflows/test.yml` to run on `push` and `pull_request`. The
workflow will install the required shell/runtime tools (`fish`, `zsh`, `fd`,
`ripgrep`, `fzf`, and `bat`) on Ubuntu, then run `tests/smoke.sh`.

## Error Handling

`tests/smoke.sh` exits non-zero on the first failed check and prints enough
context to identify the failing assertion. It should use temporary directories
for fixtures and clean them up automatically.

If optional local dependency paths are unavailable, such as both direct `zsh`
and the Docker fallback being missing, the script may skip that local check with
a clear message. GitHub Actions installs the full toolset, so CI should not skip
required syntax coverage.

## Rejected Alternatives

- **Makefile wrapper:** convenient, but it adds another project convention for a
  repository with only two source files and one test command.
- **CI-only commands:** fewer files, but local and GitHub verification would
  drift over time.
- **Full test framework:** more structure than needed for the current shell
  functions and documented smoke checks.

## Testing The Test Setup

After implementation, verify locally with:

- `tests/smoke.sh`

Then inspect the GitHub Actions workflow for parity with the local command. The
interactive fzf flow still needs a manual TTY smoke test by a user.
