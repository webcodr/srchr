# Install Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a POSIX `sh` installer that downloads the correct `srchr` release archive for Linux or macOS and installs it to `~/.local/bin` by default.

**Architecture:** Keep the installer as one root-level `install.sh` with small shell functions for dependency checks, platform detection, latest-version lookup, download, extraction, and install. Add one network-free shell test script under `tests/` that uses fake `curl`, `tar`, and `uname` commands to verify behavior.

**Tech Stack:** POSIX `sh`, `curl`, `tar`, GitHub Releases API, README Markdown, existing Rust/Cargo verification.

---

## File Structure

- Create: `install.sh` - pipe-to-shell installer for Linux/macOS release archives.
- Create: `tests/install_script_tests.sh` - shell integration tests using fake commands and temporary directories.
- Modify: `README.md:52-66` - document pipe-to-shell install, overrides, and source build fallback.
- Existing unchanged: `.github/workflows/build.yml` - already publishes the required archive names.

## Task 1: Add Installer Script

**Files:**
- Create: `install.sh`
- Test: `sh -n install.sh`

- [ ] **Step 1: Write syntax check before implementation**

Run:

```bash
sh -n install.sh
```

Expected: FAIL because `install.sh` does not exist yet.

- [ ] **Step 2: Create `install.sh`**

Create `install.sh` with this exact content:

```sh
#!/bin/sh
set -eu

REPO="webcodr/srchr"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"

err() {
  printf 'srchr install: %s\n' "$*" >&2
  exit 1
}

need_cmd() {
  command -v "$1" >/dev/null 2>&1 || err "missing required command: $1"
}

detect_os() {
  case "$(uname -s)" in
    Linux) printf 'linux' ;;
    Darwin) printf 'macos' ;;
    *) err "unsupported operating system: $(uname -s)" ;;
  esac
}

detect_arch() {
  case "$(uname -m)" in
    x86_64 | amd64) printf 'x86-64' ;;
    arm64 | aarch64) printf 'aarch64' ;;
    *) err "unsupported architecture: $(uname -m)" ;;
  esac
}

latest_version() {
  curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' \
    | sed -n '1p'
}

cleanup() {
  if [ "${tmp_dir:-}" ]; then
    rm -rf "$tmp_dir"
  fi
}

need_cmd curl
need_cmd tar
need_cmd sed
need_cmd mktemp
need_cmd uname

os="$(detect_os)"
arch="$(detect_arch)"
version="${SRCHR_VERSION:-}"

if [ -z "$version" ]; then
  version="$(latest_version)"
fi

if [ -z "$version" ]; then
  err "could not resolve latest srchr release version"
fi

asset="srchr-$version-$os-$arch.tar.gz"
url="https://github.com/$REPO/releases/download/$version/$asset"
tmp_dir="$(mktemp -d)"
trap cleanup EXIT INT TERM

printf 'Installing srchr %s for %s-%s...\n' "$version" "$os" "$arch"
curl -fsSL "$url" -o "$tmp_dir/$asset"
tar -xzf "$tmp_dir/$asset" -C "$tmp_dir"

if [ ! -f "$tmp_dir/srchr" ]; then
  err "archive did not contain srchr binary"
fi

mkdir -p "$INSTALL_DIR"
cp "$tmp_dir/srchr" "$INSTALL_DIR/srchr"
chmod 755 "$INSTALL_DIR/srchr"

printf 'Installed srchr to %s/srchr\n' "$INSTALL_DIR"

case ":$PATH:" in
  *":$INSTALL_DIR:"*) ;;
  *)
    printf 'Warning: %s is not on PATH. Add it to your shell profile to run srchr directly.\n' "$INSTALL_DIR" >&2
    ;;
esac
```

- [ ] **Step 3: Verify syntax passes**

Run:

```bash
sh -n install.sh
```

Expected: PASS with no output.

## Task 2: Add Network-Free Installer Tests

**Files:**
- Create: `tests/install_script_tests.sh`
- Modify: `install.sh` only if tests reveal a mismatch with the spec

- [ ] **Step 1: Write failing test script**

Create `tests/install_script_tests.sh` with this exact content:

```sh
#!/bin/sh
set -eu

repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

fail() {
  printf 'FAIL: %s\n' "$*" >&2
  exit 1
}

assert_file() {
  [ -f "$1" ] || fail "expected file: $1"
}

assert_contains() {
  file="$1"
  text="$2"
  if ! grep -F "$text" "$file" >/dev/null 2>&1; then
    fail "expected $file to contain: $text"
  fi
}

make_fake_bin() {
  bin_dir="$1"
  os_name="$2"
  arch_name="$3"
  mkdir -p "$bin_dir"

  cat > "$bin_dir/uname" <<EOF_UNAME
#!/bin/sh
case "\$1" in
  -s) printf '%s\n' '$os_name' ;;
  -m) printf '%s\n' '$arch_name' ;;
  *) exit 1 ;;
esac
EOF_UNAME

  cat > "$bin_dir/curl" <<'EOF_CURL'
#!/bin/sh
log_file="$SRCHR_TEST_LOG"
printf '%s\n' "$*" >> "$log_file"

case "$*" in
  *api.github.com*)
    printf '{"tag_name":"v9.8.7"}\n'
    ;;
  *github.com/webcodr/srchr/releases/download*)
    out=''
    while [ "$#" -gt 0 ]; do
      if [ "$1" = '-o' ]; then
        shift
        out="$1"
      fi
      shift || true
    done
    [ -n "$out" ] || exit 1
    printf 'fake archive\n' > "$out"
    ;;
  *)
    exit 1
    ;;
esac
EOF_CURL

  cat > "$bin_dir/tar" <<'EOF_TAR'
#!/bin/sh
dest=''
while [ "$#" -gt 0 ]; do
  if [ "$1" = '-C' ]; then
    shift
    dest="$1"
  fi
  shift || true
done
[ -n "$dest" ] || exit 1
printf '#!/bin/sh\nprintf srchr-test\\n\n' > "$dest/srchr"
EOF_TAR

  chmod +x "$bin_dir/uname" "$bin_dir/curl" "$bin_dir/tar"
}

run_installer() {
  os_name="$1"
  arch_name="$2"
  version="$3"
  tmp="$(mktemp -d)"
  bin_dir="$tmp/bin"
  install_dir="$tmp/install"
  log_file="$tmp/curl.log"
  out_file="$tmp/out.log"
  err_file="$tmp/err.log"
  make_fake_bin "$bin_dir" "$os_name" "$arch_name"

  if [ -n "$version" ]; then
    PATH="$bin_dir:$PATH" INSTALL_DIR="$install_dir" SRCHR_VERSION="$version" SRCHR_TEST_LOG="$log_file" sh "$repo_root/install.sh" >"$out_file" 2>"$err_file"
  else
    PATH="$bin_dir:$PATH" INSTALL_DIR="$install_dir" SRCHR_TEST_LOG="$log_file" sh "$repo_root/install.sh" >"$out_file" 2>"$err_file"
  fi

  assert_file "$install_dir/srchr"
  printf '%s\n' "$tmp"
}

linux_tmp="$(run_installer Linux x86_64 v1.2.3)"
assert_contains "$linux_tmp/curl.log" "https://github.com/webcodr/srchr/releases/download/v1.2.3/srchr-v1.2.3-linux-x86-64.tar.gz"
assert_contains "$linux_tmp/out.log" "Installed srchr to $linux_tmp/install/srchr"
assert_contains "$linux_tmp/err.log" "not on PATH"

mac_tmp="$(run_installer Darwin arm64 '')"
assert_contains "$mac_tmp/curl.log" "https://api.github.com/repos/webcodr/srchr/releases/latest"
assert_contains "$mac_tmp/curl.log" "https://github.com/webcodr/srchr/releases/download/v9.8.7/srchr-v9.8.7-macos-aarch64.tar.gz"

unsupported_tmp="$(mktemp -d)"
make_fake_bin "$unsupported_tmp/bin" FreeBSD x86_64
if PATH="$unsupported_tmp/bin:$PATH" INSTALL_DIR="$unsupported_tmp/install" SRCHR_VERSION=v1.2.3 SRCHR_TEST_LOG="$unsupported_tmp/curl.log" sh "$repo_root/install.sh" >"$unsupported_tmp/out.log" 2>"$unsupported_tmp/err.log"; then
  fail 'unsupported OS unexpectedly succeeded'
fi
assert_contains "$unsupported_tmp/err.log" "unsupported operating system"

printf 'install script tests passed\n'
```

- [ ] **Step 2: Run tests to verify current implementation passes or exposes issues**

Run:

```bash
sh tests/install_script_tests.sh
```

Expected: PASS with `install script tests passed`. If it fails, fix only the mismatch between `install.sh` and the approved spec, then rerun this command.

- [ ] **Step 3: Verify syntax of test script**

Run:

```bash
sh -n tests/install_script_tests.sh
```

Expected: PASS with no output.

## Task 3: Update README Install Documentation

**Files:**
- Modify: `README.md:52-68`

- [ ] **Step 1: Update install section**

Replace the current `## Install` section in `README.md` with:

````markdown
## Install

Install the latest release binary to `~/.local/bin/srchr`:

```sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | sh
```

Install a specific release or a custom directory by setting environment
variables on the `sh` side of the pipeline:

```sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | SRCHR_VERSION=v0.1.0 sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | INSTALL_DIR="$HOME/bin" sh
```

The installer supports Linux and macOS on `x86_64` and `aarch64`.

To build from source instead, build the release binary and place it somewhere on
your `PATH`:

```sh
cargo build --manifest-path rust/Cargo.toml --release
install -Dm755 rust/target/release/srchr ~/.local/bin/srchr
```
````

- [ ] **Step 2: Verify README contains expected commands**

Run:

```bash
grep -F 'curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | sh' README.md
grep -F 'SRCHR_VERSION=v0.1.0 sh' README.md
grep -F 'INSTALL_DIR="$HOME/bin" sh' README.md
```

Expected: each command prints one matching README line.

## Task 4: Final Verification

**Files:**
- Verify: `install.sh`
- Verify: `tests/install_script_tests.sh`
- Verify: `README.md`
- Verify: `rust/Cargo.toml` and Rust sources through existing gates

- [ ] **Step 1: Run installer checks**

Run:

```bash
sh -n install.sh
sh -n tests/install_script_tests.sh
sh tests/install_script_tests.sh
```

Expected: syntax checks produce no output; tests print `install script tests passed`.

- [ ] **Step 2: Run existing Rust gates**

Run:

```bash
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

Expected: all commands exit successfully.

- [ ] **Step 3: Review changed files**

Run:

```bash
git diff -- install.sh tests/install_script_tests.sh README.md docs/superpowers/specs/2026-07-07-install-script-design.md docs/superpowers/plans/2026-07-07-install-script.md
```

Expected: diff shows only the installer, installer tests, README install docs, and the design/plan docs.

- [ ] **Step 4: Commit only if explicitly requested**

Do not commit by default. If the user explicitly requests a commit, inspect `git status`, `git diff`, and `git log --oneline -10`, then commit only the intended files with:

```bash
git add install.sh tests/install_script_tests.sh README.md docs/superpowers/specs/2026-07-07-install-script-design.md docs/superpowers/plans/2026-07-07-install-script.md
git commit -m "feat: add install script"
```

## Self-Review

- Spec coverage: The plan covers root `install.sh`, POSIX shell, Linux/macOS `x86_64`/`aarch64`, default `~/.local/bin`, `INSTALL_DIR` and `SRCHR_VERSION` overrides, latest release lookup, release asset URL construction, download/extract/install flow, PATH warning, README documentation, no Windows support, no checksum verification, no sudo, and no profile modification.
- Placeholder scan: No placeholders remain; commands, file paths, script bodies, test bodies, and expected outputs are explicit.
- Type consistency: Shell variable names are consistent across installer and tests: `INSTALL_DIR`, `SRCHR_VERSION`, `REPO`, `os`, `arch`, `asset`, and `url`.
