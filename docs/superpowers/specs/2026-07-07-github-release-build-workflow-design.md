# GitHub Release Build Workflow - Design

## Summary

Add a manually triggered GitHub Actions workflow that builds `srchr` release
binaries for macOS, Linux, and Windows, publishes raw build artifacts, compresses
each binary as a `.tar.gz`, and attaches the compressed archives to a draft
GitHub release.

The workflow is modeled after
`webcodr/server-runner/.github/workflows/build.yml`, adapted for this repository's
Rust crate under `rust/` and binary name `srchr`.

## Trigger and Release

- Add `.github/workflows/build.yml` named `Build and Release`.
- Trigger only through `workflow_dispatch`.
- Require a `version` input with a default value.
- Use the `version` input exactly for both the Git tag and release name. For
  example, input `1.2.3` creates release/tag `1.2.3`, not `v1.2.3`.
- Set `target_commitish` to the workflow dispatch SHA so a newly-created tag
  points at the commit that produced the binaries.
- Create the GitHub release as a draft.
- Use `softprops/action-gh-release@v2` with `GITHUB_TOKEN`.
- Grant workflow permissions for release creation and artifact upload with
  `contents: write`.

## Build Matrix

Build six target entries matching the requested `server-runner` style coverage:

- `macos-aarch64`: `aarch64-apple-darwin`, runner `macos-latest`, artifact
  `srchr`.
- `macos-x86-64`: `x86_64-apple-darwin`, runner `macos-latest`, artifact
  `srchr`.
- `linux-aarch64`: `aarch64-unknown-linux-gnu`, runner `ubuntu-latest`, artifact
  `srchr`.
- `linux-x86-64`: `x86_64-unknown-linux-gnu`, runner `ubuntu-latest`, artifact
  `srchr`.
- `windows-aarch64`: `aarch64-pc-windows-msvc`, runner `windows-latest`, artifact
  `srchr.exe`.
- `windows-x86-64`: `x86_64-pc-windows-msvc`, runner `windows-latest`, artifact
  `srchr.exe`.

Each matrix entry checks out the repository, installs the stable Rust toolchain,
adds the target with `rustup target add`, and runs `cargo build --release` from
the `rust/` crate directory.

## Cross Compilation

Linux `aarch64` needs an additional system linker. That matrix entry installs
`gcc-aarch64-linux-gnu` and configures Cargo to use
`aarch64-linux-gnu-gcc` for `aarch64-unknown-linux-gnu`.

macOS targets build on macOS-hosted runners. Windows targets build on
Windows-hosted runners using the MSVC targets.

## Artifacts and Release Assets

Each build uploads two outputs in the workflow lifecycle:

- A raw GitHub Actions artifact containing the compiled binary.
- A compressed release asset named
  `srchr-<version>-<matrix-name>.tar.gz`.

The raw artifact name is `srchr-<version>-<matrix-name>`. The compressed archive
is created from the target release directory so the archive contains only the
binary file (`srchr` or `srchr.exe`), not the full target directory tree.

The compressed `.tar.gz` file is attached to the draft release with
`softprops/action-gh-release@v2` using the same exact input version as
`tag_name`. Asset uploads also set `draft: true` so matrix uploads do not
publish the draft release early.

## Error Handling

The workflow fails if any target fails to build, archive, upload as an artifact,
or attach to the draft release. The default matrix fail-fast behavior is
acceptable because a broken target should block a complete release.

If the release tag already exists, release creation or asset upload may fail
according to `softprops/action-gh-release` behavior. The workflow will not add
extra overwrite or deletion logic.

## Testing

This is a GitHub Actions configuration change. Local verification should include:

- YAML syntax review by inspection.
- Existing Rust gates remain unchanged in `.github/workflows/rust.yml`.
- No TUI manual smoke test is required because this does not change runtime
  behavior.

Full verification occurs by manually running the `Build and Release` workflow on
GitHub with a test version and checking that:

- A draft release is created with the exact input version as tag and name.
- Six matrix builds run.
- Six raw GitHub Actions artifacts are uploaded.
- Six `.tar.gz` release assets are attached to the draft release.

## Non-Goals

- No automatic release on Git tag push.
- No automatic `v` prefixing.
- No checks that `Cargo.toml` package version matches the workflow input.
- No publishing to package managers.
- No installer packages or zip files.
