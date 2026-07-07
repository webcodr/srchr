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
