# srchr

Self-contained live-grep file search in Rust. `srchr` searches file names and
file contents, merges the results into an interactive terminal UI, previews the
selected file with syntax highlighting, and opens your selection in `$EDITOR`.

Runtime external dependency: `$EDITOR` only.

```sh
srchr
```

- Type to run a live smart-case regex search over file names and file contents.
- Results are deduplicated file rows.
- Content hits show a match count, sorted before name-only hits.
- Name-only hits show `[name]`.
- The preview highlights the first content match with context above it, or shows
  the file from the top for name-only hits.
- Pressing `Enter` opens `$EDITOR +<line> <file>` for content hits, or
  `$EDITOR <file>` for name-only hits.

## Requirements

- Rust toolchain to build from source.
- `$EDITOR` set to an editor that understands `+<line>` for line jumps (vim,
  nvim, helix, kakoune, nano, ...).

If `$EDITOR` is unset or empty, `srchr` exits with a clear error.

## Build

```sh
cargo build --manifest-path rust/Cargo.toml --release
```

The binary is at `rust/target/release/srchr`.

## Test

```sh
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

The interactive TUI still needs a manual TTY smoke test:

```sh
cargo run --manifest-path rust/Cargo.toml -- .
```

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

## Notes

- Search respects gitignore rules.
- Search terms are smart-case regexes for both content and filename matches.
- Selected relative paths starting with `+` or `-` are normalized before
  invoking `$EDITOR`, so option/command-looking paths are treated as data.
- Preview uses an embedded default syntect theme; it does not read `bat` config
  or require `bat` to be installed.
