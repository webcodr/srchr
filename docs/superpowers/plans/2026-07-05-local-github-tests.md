# Local and GitHub Tests Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a local smoke-test script and GitHub Actions workflow for `srchr`.

**Architecture:** Keep testing lightweight: one shell script is the local and CI entry point, and one GitHub Actions workflow installs shell dependencies before running that script. The script uses temporary fixtures and command stubs to avoid requiring an interactive fzf session.

**Tech Stack:** POSIX `sh`, `bash`, `fish`, optional local `zsh` or Docker zsh fallback, GitHub Actions on Ubuntu.

---

## File Structure

- Create: `tests/smoke.sh`
  - Single local test entry point.
  - Owns syntax checks, fzf argument parity checks, direct snippet checks, and security smoke checks.
  - Uses temporary stub commands for `fd`, `rg`, `fzf`, `bat`, and `$EDITOR`.
- Create: `.github/workflows/test.yml`
  - Runs `tests/smoke.sh` on `push` and `pull_request`.
  - Installs `fish`, `zsh`, `fd`, `ripgrep`, `fzf`, and `bat` equivalents on Ubuntu.
- Modify: `README.md`
  - Add a short `Test` section showing `tests/smoke.sh`.
- Modify: `AGENTS.md`
  - Replace “No build system, no test suite, no CI.” with current status.
  - Point verification instructions at `tests/smoke.sh` while preserving the manual TTY note.

## Task 1: Add Local Smoke Test Script

**Files:**
- Create: `tests/smoke.sh`

- [ ] **Step 1: Create the test script with syntax, parity, snippet, and security checks**

Create `tests/smoke.sh` with this content:

```sh
#!/bin/sh
set -eu

ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
TMPDIR=${TMPDIR:-/tmp}
WORK=$(mktemp -d "$TMPDIR/srchr-tests.XXXXXX")

cleanup() {
    rm -rf "$WORK"
}
trap cleanup EXIT INT TERM

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

pass() {
    printf 'PASS: %s\n' "$*"
}

skip() {
    printf 'SKIP: %s\n' "$*"
}

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

assert_eq() {
    label=$1
    expected=$2
    actual=$3

    if [ "$expected" != "$actual" ]; then
        printf 'Expected %s:\n%s\n\nActual %s:\n%s\n' "$label" "$expected" "$label" "$actual" >&2
        fail "$label mismatch"
    fi
}

assert_contains() {
    label=$1
    haystack=$2
    needle=$3

    case $haystack in
        *"$needle"*) ;;
        *)
            printf 'Expected %s to contain:\n%s\n\nActual:\n%s\n' "$label" "$needle" "$haystack" >&2
            fail "$label missing expected text"
            ;;
    esac
}

write_executable() {
    path=$1
    content=$2
    printf '%s\n' "$content" >"$path"
    chmod +x "$path"
}

syntax_checks() {
    command_exists fish || fail 'fish is required for srchr.fish syntax checks'
    fish -n "$ROOT/srchr.fish"
    pass 'fish syntax'

    bash -n "$ROOT/srchr.sh"
    pass 'bash syntax'

    if command_exists zsh; then
        zsh -n "$ROOT/srchr.sh"
        pass 'zsh syntax'
    elif command_exists docker; then
        docker run --rm -v "$ROOT:/w" -w /w zshusers/zsh zsh -n srchr.sh
        pass 'zsh syntax via Docker'
    else
        skip 'zsh syntax check; install zsh or docker to run it locally'
    fi
}

make_capture_stubs() {
    stub_dir=$1
    mkdir -p "$stub_dir"

    write_executable "$stub_dir/fd" '#!/bin/sh
printf "fd" >>"$SRCHR_CMD_LOG"
for arg do printf " [%s]" "$arg" >>"$SRCHR_CMD_LOG"; done
printf "\n" >>"$SRCHR_CMD_LOG"
printf "%s\n" "file-from-fd"'

    write_executable "$stub_dir/rg" '#!/bin/sh
printf "rg" >>"$SRCHR_CMD_LOG"
for arg do printf " [%s]" "$arg" >>"$SRCHR_CMD_LOG"; done
printf "\n" >>"$SRCHR_CMD_LOG"
printf "%s\n" "file-from-rg"'

    write_executable "$stub_dir/fzf" '#!/bin/sh
: >"$SRCHR_FZF_ARGS"
for arg do printf "%s\n" "$arg" >>"$SRCHR_FZF_ARGS"; done
cat >"$SRCHR_FZF_INPUT"'
}

capture_sh_args() {
    out_dir=$1
    stub_dir=$out_dir/bin
    mkdir -p "$out_dir"
    make_capture_stubs "$stub_dir"

    SRCHR_CMD_LOG=$out_dir/commands.log \
    SRCHR_FZF_ARGS=$out_dir/fzf.args \
    SRCHR_FZF_INPUT=$out_dir/fzf.input \
    PATH="$stub_dir:$PATH" \
    bash -c '. "$1"; srchr "$2"; [ -z "${SRCHR_TERM+x}" ]' sh "$ROOT/srchr.sh" '--exec=rm'
}

capture_fish_args() {
    out_dir=$1
    stub_dir=$out_dir/bin
    mkdir -p "$out_dir"
    make_capture_stubs "$stub_dir"

    SRCHR_CMD_LOG=$out_dir/commands.log \
    SRCHR_FZF_ARGS=$out_dir/fzf.args \
    SRCHR_FZF_INPUT=$out_dir/fzf.input \
    PATH="$stub_dir:$PATH" \
    fish -c 'source "$argv[1]"; srchr "$argv[2]"; if set -q SRCHR_TERM; exit 7; end' "$ROOT/srchr.fish" '--exec=rm'
}

line_after() {
    file=$1
    marker=$2
    found=0

    while IFS= read -r line; do
        if [ "$found" -eq 1 ]; then
            printf '%s' "$line"
            return 0
        fi
        if [ "$line" = "$marker" ]; then
            found=1
        fi
    done <"$file"

    fail "argument after $marker not found in $file"
}

extract_preview_snippet() {
    arg=$1
    prefix="sh -c '"
    suffix="' sh {}"
    body=${arg#"$prefix"}
    body=${body%"$suffix"}
    [ "$body" != "$arg" ] || fail 'preview argument did not match expected sh -c shape'
    printf '%s' "$body"
}

extract_open_snippet() {
    arg=$1
    prefix="enter:become(sh -c '"
    suffix="' sh {})"
    body=${arg#"$prefix"}
    body=${body%"$suffix"}
    [ "$body" != "$arg" ] || fail 'enter binding did not match expected become shape'
    printf '%s' "$body"
}

check_port_parity_and_injection() {
    sh_dir=$WORK/sh-capture
    fish_dir=$WORK/fish-capture

    capture_sh_args "$sh_dir"
    capture_fish_args "$fish_dir"

    sh_preview=$(line_after "$sh_dir/fzf.args" '--preview')
    fish_preview=$(line_after "$fish_dir/fzf.args" '--preview')
    sh_bind=$(line_after "$sh_dir/fzf.args" '--bind')
    fish_bind=$(line_after "$fish_dir/fzf.args" '--bind')

    assert_eq 'preview fzf argument' "$sh_preview" "$fish_preview"
    assert_eq 'enter fzf binding' "$sh_bind" "$fish_bind"

    sh_commands=$(cat "$sh_dir/commands.log")
    fish_commands=$(cat "$fish_dir/commands.log")
    assert_contains 'sh fd arguments' "$sh_commands" 'fd [-tf] [--] [--exec=rm]'
    assert_contains 'sh rg arguments' "$sh_commands" 'rg [-lS] [--] [--exec=rm]'
    assert_contains 'fish fd arguments' "$fish_commands" 'fd [-tf] [--] [--exec=rm]'
    assert_contains 'fish rg arguments' "$fish_commands" 'rg [-lS] [--] [--exec=rm]'

    pass 'fzf argument parity and leading-option search term safety'

    PREVIEW_SNIPPET=$(extract_preview_snippet "$sh_preview")
    OPEN_SNIPPET=$(extract_open_snippet "$sh_bind")
    export PREVIEW_SNIPPET OPEN_SNIPPET
}

make_snippet_stubs() {
    stub_dir=$1
    mode=$2
    mkdir -p "$stub_dir"

    write_executable "$stub_dir/rg" '#!/bin/sh
last=
for arg do last=$arg; done
printf "rg" >>"$SRCHR_SNIPPET_LOG"
for arg do printf " [%s]" "$arg" >>"$SRCHR_SNIPPET_LOG"; done
printf "\n" >>"$SRCHR_SNIPPET_LOG"
case "$SRCHR_RG_MODE:$last" in
    match:*|match-normalized:./+*|match-normalized:./-*) printf "%s:7:needle\n" "$last" ;;
esac'

    write_executable "$stub_dir/bat" '#!/bin/sh
printf "bat" >>"$SRCHR_SNIPPET_LOG"
for arg do printf " [%s]" "$arg" >>"$SRCHR_SNIPPET_LOG"; done
printf "\n" >>"$SRCHR_SNIPPET_LOG"'

    write_executable "$stub_dir/editor" '#!/bin/sh
printf "editor" >>"$SRCHR_SNIPPET_LOG"
for arg do printf " [%s]" "$arg" >>"$SRCHR_SNIPPET_LOG"; done
printf "\n" >>"$SRCHR_SNIPPET_LOG"'

    printf '%s' "$mode" >"$stub_dir/mode"
}

run_preview_snippet() {
    label=$1
    mode=$2
    file=$3
    out_dir=$WORK/snippet-$label
    stub_dir=$out_dir/bin
    mkdir -p "$out_dir"
    make_snippet_stubs "$stub_dir" "$mode"

    : >"$out_dir/log"
    SRCHR_TERM=needle \
    SRCHR_RG_MODE=$mode \
    SRCHR_SNIPPET_LOG=$out_dir/log \
    PATH="$stub_dir:$PATH" \
    sh -c "$PREVIEW_SNIPPET" sh "$file"
    cat "$out_dir/log"
}

run_open_snippet() {
    label=$1
    mode=$2
    file=$3
    out_dir=$WORK/open-$label
    stub_dir=$out_dir/bin
    mkdir -p "$out_dir"
    make_snippet_stubs "$stub_dir" "$mode"

    : >"$out_dir/log"
    SRCHR_TERM=needle \
    SRCHR_RG_MODE=$mode \
    SRCHR_SNIPPET_LOG=$out_dir/log \
    EDITOR=$stub_dir/editor \
    PATH="$stub_dir:$PATH" \
    sh -c "$OPEN_SNIPPET" sh "$file"
    cat "$out_dir/log"
}

check_snippets() {
    preview_match=$(run_preview_snippet preview-match match match-file)
    assert_contains 'preview match rg call' "$preview_match" 'rg [-nS] [-m1] [--] [needle] [match-file]'
    assert_contains 'preview match bat call' "$preview_match" 'bat [--color] [always] [--highlight-line] [7] [--line-range] [4:] [match-file]'

    preview_nomatch=$(run_preview_snippet preview-nomatch nomatch plain-file)
    assert_contains 'preview no-match bat call' "$preview_nomatch" 'bat [--color] [always] [plain-file]'

    open_match=$(run_open_snippet open-match match match-file)
    assert_contains 'open match editor call' "$open_match" 'editor [+7] [match-file]'

    open_nomatch=$(run_open_snippet open-nomatch nomatch plain-file)
    assert_contains 'open no-match editor call' "$open_nomatch" 'editor [plain-file]'

    plus_path=$(run_open_snippet open-plus match-normalized '+!touch pwned')
    assert_contains 'leading-plus rg normalization' "$plus_path" 'rg [-nS] [-m1] [--] [needle] [./+!touch pwned]'
    assert_contains 'leading-plus editor normalization' "$plus_path" 'editor [+7] [./+!touch pwned]'

    dash_path=$(run_preview_snippet preview-dash match-normalized '-looks-like-option')
    assert_contains 'leading-dash rg normalization' "$dash_path" 'rg [-nS] [-m1] [--] [needle] [./-looks-like-option]'
    assert_contains 'leading-dash bat normalization' "$dash_path" 'bat [--color] [always] [--highlight-line] [7] [--line-range] [4:] [./-looks-like-option]'

    pass 'preview, enter, and selected-path security snippets'
}

syntax_checks
check_port_parity_and_injection
check_snippets

printf 'All smoke tests passed.\n'
```

- [ ] **Step 2: Make the script executable**

Run:

```bash
chmod +x tests/smoke.sh
```

Expected: no output and `tests/smoke.sh` is executable.

- [ ] **Step 3: Run the new smoke test locally**

Run:

```bash
tests/smoke.sh
```

Expected: the script prints `PASS:` lines and ends with `All smoke tests passed.` If local `zsh` and Docker are both missing, the script may print one `SKIP:` line for zsh syntax.

- [ ] **Step 4: Inspect changed files instead of committing**

Run:

```bash
git status --short
git diff -- tests/smoke.sh
```

Expected: `tests/smoke.sh` is listed as new, and the diff contains only the smoke-test script. Do not commit unless the user explicitly asks for a commit.

## Task 2: Add GitHub Actions Workflow

**Files:**
- Create: `.github/workflows/test.yml`

- [ ] **Step 1: Create the workflow**

Create `.github/workflows/test.yml` with this content:

```yaml
name: Test

on:
  push:
  pull_request:

jobs:
  smoke:
    runs-on: ubuntu-latest

    steps:
      - name: Check out repository
        uses: actions/checkout@v4

      - name: Install shell and search dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y fish zsh fd-find ripgrep fzf bat
          sudo ln -sf "$(command -v fdfind)" /usr/local/bin/fd
          sudo ln -sf "$(command -v batcat)" /usr/local/bin/bat

      - name: Run smoke tests
        run: tests/smoke.sh
```

- [ ] **Step 2: Run the local test again after adding CI**

Run:

```bash
tests/smoke.sh
```

Expected: the same result as Task 1, with all available local checks passing.

- [ ] **Step 3: Inspect workflow diff instead of committing**

Run:

```bash
git status --short
git diff -- .github/workflows/test.yml
```

Expected: `.github/workflows/test.yml` is listed as new, and the workflow installs dependencies then runs `tests/smoke.sh`. Do not commit unless the user explicitly asks for a commit.

## Task 3: Update Project Documentation

**Files:**
- Modify: `README.md`
- Modify: `AGENTS.md`

- [ ] **Step 1: Add a README test section**

In `README.md`, add this section after the `Requirements` section and before `Install`:

````markdown
## Test

Run the local smoke checks:

```sh
tests/smoke.sh
```

The interactive fzf flow still needs a manual TTY smoke test.
````

- [ ] **Step 2: Update AGENTS project status and verification guidance**

Change `AGENTS.md` line 4 from:

```markdown
No build system, no test suite, no CI.
```

to:

```markdown
No build system. Local smoke tests live in `tests/smoke.sh`; GitHub Actions runs the same script.
```

Then replace the `## Verification` section with:

```markdown
## Verification

- Run the automated smoke suite: `tests/smoke.sh`
- The script covers syntax checks, fzf preview/bind parity across shell ports,
  direct snippet behavior, and security smoke checks for leading-option terms
  and selected paths beginning with `+` or `-`.
- zsh is optional locally: the script uses direct `zsh` when installed, falls
  back to Docker when available, and skips only when neither exists.
- The interactive fzf flow needs a TTY; the agent cannot test it — ask the
  user for a manual smoke test.
```

- [ ] **Step 3: Run smoke tests after docs changes**

Run:

```bash
tests/smoke.sh
```

Expected: all available local checks still pass.

- [ ] **Step 4: Inspect documentation diff instead of committing**

Run:

```bash
git diff -- README.md AGENTS.md
```

Expected: README documents `tests/smoke.sh`; AGENTS reflects the new local and CI test setup. Do not commit unless the user explicitly asks for a commit.

## Task 4: Final Verification

**Files:**
- Verify: all changed files

- [ ] **Step 1: Run the project smoke suite**

Run:

```bash
tests/smoke.sh
```

Expected: `All smoke tests passed.` Local zsh syntax may be skipped only if neither `zsh` nor Docker is available.

- [ ] **Step 2: Run explicit shell syntax commands for visible evidence**

Run:

```bash
fish -n srchr.fish
bash -n srchr.sh
```

Expected: both commands exit with no output.

- [ ] **Step 3: Run zsh syntax directly or through Docker**

Run direct zsh when available:

```bash
zsh -n srchr.sh
```

Expected: exits with no output.

If direct zsh is unavailable but Docker is available, run:

```bash
docker run --rm -v "$PWD:/w" -w /w zshusers/zsh zsh -n srchr.sh
```

Expected: exits with no output.

- [ ] **Step 4: Review final worktree state**

Run:

```bash
git status --short
git diff --stat
```

Expected: changed files are limited to the test script, GitHub workflow, docs updates, and superpowers planning/spec files. Do not commit unless the user explicitly asks for a commit.

## Self-Review Notes

- Spec coverage: the plan creates one local script, one GitHub workflow, keeps interactive fzf manual, avoids product behavior changes, and documents the new test entry point.
- Placeholder scan: no prohibited placeholder markers or unspecified implementation steps remain.
- Consistency check: all commands use `tests/smoke.sh`; the CI workflow runs the same local entry point.
