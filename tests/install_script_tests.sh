#!/bin/sh
set -eu

# shellcheck disable=SC1007
repo_root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"

tmp_dirs_file="$(mktemp -d)/tmp_dirs.list"
: > "$tmp_dirs_file"

cleanup_tmp_dirs() {
  if [ -f "$tmp_dirs_file" ]; then
    while IFS= read -r dir; do
      [ -n "$dir" ] && rm -rf "$dir"
    done < "$tmp_dirs_file"
    rm -rf "$(dirname -- "$tmp_dirs_file")"
  fi
}
trap cleanup_tmp_dirs EXIT INT TERM

track_tmp_dir() {
  printf '%s\n' "$1" >> "$tmp_dirs_file"
}

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
  skip_binary="${4:-}"
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

  if [ "$skip_binary" = 'skip-binary' ]; then
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
# Intentionally does not write a srchr binary, to simulate a bad archive.
EOF_TAR
  else
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
  fi

  chmod +x "$bin_dir/uname" "$bin_dir/curl" "$bin_dir/tar"
}

run_installer() {
  os_name="$1"
  arch_name="$2"
  version="$3"
  tmp="$(mktemp -d)"
  track_tmp_dir "$tmp"
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
track_tmp_dir "$unsupported_tmp"
make_fake_bin "$unsupported_tmp/bin" FreeBSD x86_64
if PATH="$unsupported_tmp/bin:$PATH" INSTALL_DIR="$unsupported_tmp/install" SRCHR_VERSION=v1.2.3 SRCHR_TEST_LOG="$unsupported_tmp/curl.log" sh "$repo_root/install.sh" >"$unsupported_tmp/out.log" 2>"$unsupported_tmp/err.log"; then
  fail 'unsupported OS unexpectedly succeeded'
fi
assert_contains "$unsupported_tmp/err.log" "unsupported operating system"

unsupported_arch_tmp="$(mktemp -d)"
track_tmp_dir "$unsupported_arch_tmp"
make_fake_bin "$unsupported_arch_tmp/bin" Linux mips
if PATH="$unsupported_arch_tmp/bin:$PATH" INSTALL_DIR="$unsupported_arch_tmp/install" SRCHR_VERSION=v1.2.3 SRCHR_TEST_LOG="$unsupported_arch_tmp/curl.log" sh "$repo_root/install.sh" >"$unsupported_arch_tmp/out.log" 2>"$unsupported_arch_tmp/err.log"; then
  fail 'unsupported architecture unexpectedly succeeded'
fi
assert_contains "$unsupported_arch_tmp/err.log" "unsupported architecture"

bad_archive_tmp="$(mktemp -d)"
track_tmp_dir "$bad_archive_tmp"
make_fake_bin "$bad_archive_tmp/bin" Linux x86_64 skip-binary
if PATH="$bad_archive_tmp/bin:$PATH" INSTALL_DIR="$bad_archive_tmp/install" SRCHR_VERSION=v1.2.3 SRCHR_TEST_LOG="$bad_archive_tmp/curl.log" sh "$repo_root/install.sh" >"$bad_archive_tmp/out.log" 2>"$bad_archive_tmp/err.log"; then
  fail 'bad archive unexpectedly succeeded'
fi
assert_contains "$bad_archive_tmp/err.log" "did not contain srchr binary"

printf 'install script tests passed\n'
