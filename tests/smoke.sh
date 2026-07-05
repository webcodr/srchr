#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)
tmp_root=${TMPDIR:-/tmp}
work=$(mktemp -d "$tmp_root/srchr-smoke.XXXXXXXXXX")
trap 'rm -rf "$work"' EXIT

fail() {
    printf 'FAIL: %s\n' "$*" >&2
    exit 1
}

pass() {
    printf 'PASS: %s\n' "$*"
}

assert_eq() {
    local actual=$1
    local expected=$2
    local context=$3

    if [[ $actual != "$expected" ]]; then
        printf 'FAIL: %s\n' "$context" >&2
        printf 'expected: [%s]\n' "$expected" >&2
        printf 'actual:   [%s]\n' "$actual" >&2
        exit 1
    fi
}

assert_file_contains_line() {
    local file=$1
    local expected=$2
    local context=$3
    local line

    while IFS= read -r line; do
        [[ $line == "$expected" ]] && return 0
    done < "$file"

    printf 'FAIL: %s\n' "$context" >&2
    printf 'missing line: [%s]\n' "$expected" >&2
    printf 'file contents:\n' >&2
    while IFS= read -r line; do
        printf '  [%s]\n' "$line" >&2
    done < "$file"
    exit 1
}

assert_file_lacks_text() {
    local file=$1
    local text=$2
    local context=$3
    local line

    while IFS= read -r line; do
        if [[ $line == *"$text"* ]]; then
            printf 'FAIL: %s\n' "$context" >&2
            printf 'unexpected line: [%s]\n' "$line" >&2
            exit 1
        fi
    done < "$file"
}

assert_lacks_text() {
    local actual=$1
    local text=$2
    local context=$3

    if [[ $actual == *"$text"* ]]; then
        printf 'FAIL: %s\n' "$context" >&2
        printf 'unexpected text: [%s]\n' "$text" >&2
        printf 'actual:          [%s]\n' "$actual" >&2
        exit 1
    fi
}

write_stubs() {
    local bin=$1
    mkdir -p "$bin"

    cat > "$bin/fd" <<'STUB'
#!/bin/sh
{
    printf 'fd'
    for arg do printf '\t%s' "$arg"; done
    printf '\n'
} >> "$CALL_LOG"
printf '%s\n' 'match-file'
STUB

    cat > "$bin/rg" <<'STUB'
#!/bin/sh
case "$1" in
    -lS)
        {
            printf 'rg_search'
            for arg do printf '\t%s' "$arg"; done
            printf '\n'
        } >> "$CALL_LOG"
        printf '%s\n' 'match-file'
        ;;
    -nS)
        {
            printf 'rg_snippet'
            for arg do printf '\t%s' "$arg"; done
            printf '\n'
        } >> "$CALL_LOG"
        file=$5
        case "$file" in
            *nomatch*) exit 1 ;;
            *) printf '7:matched\n' ;;
        esac
        ;;
    *)
        printf 'unexpected rg invocation:' >&2
        for arg do printf ' [%s]' "$arg" >&2; done
        printf '\n' >&2
        exit 1
        ;;
esac
STUB

    cat > "$bin/fzf" <<'STUB'
#!/bin/sh
: > "$FZF_ARGS_FILE"
for arg do printf '%s\n' "$arg" >> "$FZF_ARGS_FILE"; done
cat >/dev/null
STUB

    cat > "$bin/bat" <<'STUB'
#!/bin/sh
{
    printf 'bat'
    for arg do printf '\t%s' "$arg"; done
    printf '\n'
} >> "$CALL_LOG"
STUB

    cat > "$bin/stub-editor" <<'STUB'
#!/bin/sh
{
    printf 'editor'
    for arg do printf '\t%s' "$arg"; done
    printf '\n'
} >> "$CALL_LOG"
STUB

    chmod +x "$bin/fd" "$bin/rg" "$bin/fzf" "$bin/bat" "$bin/stub-editor"
}

capture_sh() {
    local term=$1
    local out=$2
    mkdir -p "$out"
    : > "$out/calls.log"
    : > "$out/fzf.args"

    env -u SRCHR_TERM PATH="$STUB_BIN:$PATH" CALL_LOG="$out/calls.log" FZF_ARGS_FILE="$out/fzf.args" \
        bash -c '. ./srchr.sh; srchr "$1"; if [ "${SRCHR_TERM+x}" = x ]; then printf "%s\n" "SRCHR_TERM leaked after bash invocation" >&2; exit 1; fi' sh "$term"
}

capture_fish() {
    local term=$1
    local out=$2
    mkdir -p "$out"
    : > "$out/calls.log"
    : > "$out/fzf.args"

    env -u SRCHR_TERM PATH="$STUB_BIN:$PATH" CALL_LOG="$out/calls.log" FZF_ARGS_FILE="$out/fzf.args" \
        fish --no-config -c 'source srchr.fish; srchr $argv[1]; if set -q SRCHR_TERM; printf "%s\n" "SRCHR_TERM leaked after fish invocation" >&2; exit 1; end' -- "$term"
}

arg_after() {
    local file=$1
    local flag=$2
    local previous=
    local line

    while IFS= read -r line; do
        if [[ $previous == "$flag" ]]; then
            printf '%s' "$line"
            return 0
        fi
        previous=$line
    done < "$file"

    fail "missing $flag in $file"
}

preview_snippet_from() {
    local cmd=$1
    local prefix="sh -c '"
    local suffix="' sh {}"

    [[ $cmd == "$prefix"*"$suffix" ]] || fail "unexpected preview command: $cmd"
    cmd=${cmd#"$prefix"}
    cmd=${cmd%"$suffix"}
    printf '%s' "$cmd"
}

open_snippet_from() {
    local cmd=$1
    local prefix="enter:become(sh -c '"
    local suffix="' sh {})"

    [[ $cmd == "$prefix"*"$suffix" ]] || fail "unexpected bind command: $cmd"
    cmd=${cmd#"$prefix"}
    cmd=${cmd%"$suffix"}
    printf '%s' "$cmd"
}

run_preview() {
    local snippet=$1
    local selected=$2
    local log=$3

    : > "$log"
    env PATH="$STUB_BIN:$PATH" CALL_LOG="$log" SRCHR_TERM='needle' \
        sh -c "$snippet" sh "$selected"
}

run_open() {
    local snippet=$1
    local selected=$2
    local log=$3

    : > "$log"
    env PATH="$STUB_BIN:$PATH" CALL_LOG="$log" SRCHR_TERM='needle' EDITOR=stub-editor \
        sh -c "$snippet" sh "$selected"
}

cd "$ROOT"

command -v fish >/dev/null 2>&1 || fail 'fish is required for fish -n srchr.fish'
fish --no-config -n srchr.fish
pass 'fish syntax'

bash -n srchr.sh
pass 'bash syntax'

if command -v zsh >/dev/null 2>&1; then
    zsh -n srchr.sh
    pass 'zsh syntax'
elif command -v docker >/dev/null 2>&1; then
    docker run --rm -v "$PWD:/w" -w /w zshusers/zsh zsh -n srchr.sh
    pass 'zsh syntax via Docker'
else
    printf 'SKIP: zsh syntax (zsh and docker unavailable)\n'
fi

STUB_BIN="$work/bin"
write_stubs "$STUB_BIN"

SH_OUT="$work/sh"
FISH_OUT="$work/fish"
capture_sh '--exec=rm' "$SH_OUT"
capture_fish '--exec=rm' "$FISH_OUT"
pass 'SRCHR_TERM remains local to each srchr invocation'

assert_file_contains_line "$SH_OUT/calls.log" $'fd\t-tf\t--\t--exec=rm' 'bash fd search term must be passed after --'
assert_file_contains_line "$SH_OUT/calls.log" $'rg_search\t-lS\t--\t--exec=rm' 'bash rg search term must be passed after --'
assert_file_contains_line "$FISH_OUT/calls.log" $'fd\t-tf\t--\t--exec=rm' 'fish fd search term must be passed after --'
assert_file_contains_line "$FISH_OUT/calls.log" $'rg_search\t-lS\t--\t--exec=rm' 'fish rg search term must be passed after --'
pass 'search terms that look like options are passed after --'

INJECTION_TERM=$'needle\'";touch pwned'
SH_INJECTION_OUT="$work/sh-injection"
FISH_INJECTION_OUT="$work/fish-injection"
capture_sh "$INJECTION_TERM" "$SH_INJECTION_OUT"
capture_fish "$INJECTION_TERM" "$FISH_INJECTION_OUT"

sh_injection_preview_arg=$(arg_after "$SH_INJECTION_OUT/fzf.args" '--preview')
fish_injection_preview_arg=$(arg_after "$FISH_INJECTION_OUT/fzf.args" '--preview')
sh_injection_bind_arg=$(arg_after "$SH_INJECTION_OUT/fzf.args" '--bind')
fish_injection_bind_arg=$(arg_after "$FISH_INJECTION_OUT/fzf.args" '--bind')

assert_lacks_text "$sh_injection_preview_arg" "$INJECTION_TERM" 'bash preview fzf arg must not interpolate the search term'
assert_lacks_text "$fish_injection_preview_arg" "$INJECTION_TERM" 'fish preview fzf arg must not interpolate the search term'
assert_lacks_text "$sh_injection_bind_arg" "$INJECTION_TERM" 'bash bind fzf arg must not interpolate the search term'
assert_lacks_text "$fish_injection_bind_arg" "$INJECTION_TERM" 'fish bind fzf arg must not interpolate the search term'
pass 'fzf preview/bind args receive search term only through SRCHR_TERM'

sh_preview_arg=$(arg_after "$SH_OUT/fzf.args" '--preview')
fish_preview_arg=$(arg_after "$FISH_OUT/fzf.args" '--preview')
sh_bind_arg=$(arg_after "$SH_OUT/fzf.args" '--bind')
fish_bind_arg=$(arg_after "$FISH_OUT/fzf.args" '--bind')

assert_eq "$fish_preview_arg" "$sh_preview_arg" 'fish and sh preview fzf args must match byte-for-byte'
assert_eq "$fish_bind_arg" "$sh_bind_arg" 'fish and sh bind fzf args must match byte-for-byte'
pass 'fish and sh fzf preview/bind args match'

preview_snippet=$(preview_snippet_from "$sh_preview_arg")
open_snippet=$(open_snippet_from "$sh_bind_arg")
DIRECT_LOG="$work/direct.log"

run_preview "$preview_snippet" 'match-file' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\tmatch-file' 'preview match must search selected file'
assert_file_contains_line "$DIRECT_LOG" $'bat\t--color\talways\t--highlight-line\t7\t--line-range\t4:\tmatch-file' 'preview match must highlight first matched line'

run_preview "$preview_snippet" 'nomatch-file' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\tnomatch-file' 'preview no-match must search selected file'
assert_file_contains_line "$DIRECT_LOG" $'bat\t--color\talways\tnomatch-file' 'preview no-match must show the whole file'
assert_file_lacks_text "$DIRECT_LOG" '--highlight-line' 'preview no-match must not request a highlighted line'
pass 'preview snippet handles match and no-match cases'

run_open "$open_snippet" 'match-file' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\tmatch-file' 'open match must search selected file'
assert_file_contains_line "$DIRECT_LOG" $'editor\t+7\tmatch-file' 'open match must pass +line to editor'

run_open "$open_snippet" 'nomatch-file' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\tnomatch-file' 'open no-match must search selected file'
assert_file_contains_line "$DIRECT_LOG" $'editor\tnomatch-file' 'open no-match must pass only the file to editor'
pass 'open snippet handles match and no-match cases'

run_preview "$preview_snippet" '+!touch pwned-match' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\t./+!touch pwned-match' 'preview must normalize leading + before rg while preserving selected path as one argument'
assert_file_contains_line "$DIRECT_LOG" $'bat\t--color\talways\t--highlight-line\t7\t--line-range\t4:\t./+!touch pwned-match' 'preview must normalize leading + before bat while preserving selected path as one argument'

run_preview "$preview_snippet" '-dash match' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\t./-dash match' 'preview must normalize leading - before rg while preserving selected path as one argument'
assert_file_contains_line "$DIRECT_LOG" $'bat\t--color\talways\t--highlight-line\t7\t--line-range\t4:\t./-dash match' 'preview must normalize leading - before bat while preserving selected path as one argument'

run_open "$open_snippet" '+!touch pwned-match' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\t./+!touch pwned-match' 'open must normalize leading + before rg while preserving selected path as one argument'
assert_file_contains_line "$DIRECT_LOG" $'editor\t+7\t./+!touch pwned-match' 'open must normalize leading + before editor while preserving selected path as one argument'

run_open "$open_snippet" '-dash match' "$DIRECT_LOG"
assert_file_contains_line "$DIRECT_LOG" $'rg_snippet\t-nS\t-m1\t--\tneedle\t./-dash match' 'open must normalize leading - before rg while preserving selected path as one argument'
assert_file_contains_line "$DIRECT_LOG" $'editor\t+7\t./-dash match' 'open must normalize leading - before editor while preserving selected path as one argument'
pass 'selected paths beginning with + or - are normalized before tool invocation'

printf 'All smoke tests passed.\n'
