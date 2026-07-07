# Install Script - Design

## Summary

Add a small pipe-to-shell installer for `srchr` so users can install the latest
published binary with one command:

```sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | sh
```

The installer supports Linux and macOS on `x86_64` and `aarch64`, matching the
release assets produced by the GitHub release workflow.

## Installer Location

- Add `install.sh` at the repository root.
- Keep it POSIX `sh` compatible.
- Document the install command in `README.md`.

## Defaults and Overrides

- Install to `${INSTALL_DIR:-$HOME/.local/bin}`.
- Allow version override with `SRCHR_VERSION`, for example:

```sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | SRCHR_VERSION=v0.1.0 sh
```

- If `SRCHR_VERSION` is unset, query the GitHub Releases API for the latest
  release tag.

## Platform Detection

The script detects the release asset suffix from `uname`:

- `uname -s` `Linux` -> `linux`.
- `uname -s` `Darwin` -> `macos`.
- Other operating systems fail with a clear unsupported platform message.
- `uname -m` `x86_64` or `amd64` -> `x86-64`.
- `uname -m` `arm64` or `aarch64` -> `aarch64`.
- Other architectures fail with a clear unsupported architecture message.

The final asset name is:

```text
srchr-<version>-<os>-<arch>.tar.gz
```

For example:

```text
srchr-v0.1.0-linux-x86-64.tar.gz
srchr-v0.1.0-macos-aarch64.tar.gz
```

## Install Flow

The script will:

- Enable `set -eu`.
- Require `curl` and `tar`.
- Resolve `SRCHR_VERSION` from the latest release when unset.
- Create a temporary directory with `mktemp -d`.
- Download the matching release archive with `curl -fsSL`.
- Extract the archive with `tar -xzf`.
- Create `INSTALL_DIR` when missing.
- Install `srchr` to `$INSTALL_DIR/srchr` with executable permissions.
- Remove the temporary directory on exit.
- Print the installed path.
- Warn if `INSTALL_DIR` is not on `PATH`.

## Error Handling

- Missing `curl` or `tar` exits with a clear error.
- Unsupported OS or architecture exits with a clear error.
- Failed latest-release lookup exits with a clear error.
- Failed download, extraction, directory creation, or install exits non-zero.
- The script does not attempt `sudo`; users can override `INSTALL_DIR` if they
  want a different location.

## Documentation

Update `README.md` install instructions to show:

```sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | sh
```

Also document optional overrides:

```sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | SRCHR_VERSION=v0.1.0 sh
curl -fsSL https://raw.githubusercontent.com/webcodr/srchr/main/install.sh | INSTALL_DIR="$HOME/bin" sh
```

The environment assignments are intentionally placed on the `sh` side of the
pipeline so they are available to the installer process.

Keep the existing source-build instructions as a fallback.

## Testing

Local verification should include:

- `sh -n install.sh`.
- A shell integration test using temporary fake `curl`, `tar`, and `uname`
  commands to verify platform mapping, URL construction, installation into a
  temporary `INSTALL_DIR`, and PATH warnings without making network requests.
- Existing Rust verification:

```sh
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

## Non-Goals

- No Windows support in the pipe-to-shell installer.
- No checksum verification in this first version.
- No package-manager integration.
- No automatic `sudo` escalation.
- No shell profile modification.
