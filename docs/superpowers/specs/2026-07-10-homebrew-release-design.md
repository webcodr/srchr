# Homebrew Release Design

## Summary

Publish `srchr` through `webcodr/homebrew-tap` using a generated formula that
references the platform-specific archives attached to a published GitHub
release. The project distributes prebuilt binaries, so the formula installs the
archive's top-level `srchr` binary rather than building Rust sources.

## Release Contract

- Stable release tags use `vX.Y.Z` and must match `rust/Cargo.toml`'s package
  version without the leading `v`.
- The existing build workflow attaches the following archives to each draft
  release: macOS aarch64/x86_64 and Linux aarch64/x86_64. Windows archives are
  not represented because Homebrew only supports macOS and Linux.
- The update workflow runs only after a release is published. This ensures the
  public formula never refers to inaccessible draft-release assets.

## Formula Template

`Formula/srchr.rb` is a checked-in template. Its `VERSION`, `TAG`, and SHA-256
placeholders are rendered by the workflow. The rendered formula has per-OS and
per-CPU `url`/`sha256` pairs, installs `srchr` into `bin`, and checks the
non-interactive `--help` output because the regular application flow requires a
TTY.

## Tap Update Workflow

`.github/workflows/update-homebrew.yml` listens for a published release and
also supports manual dispatch for a published-tag repair or backfill. It:

1. checks out the formula template from the default branch, which also permits
   backfilling a release created before the template existed;
2. validates the release tag and confirms that it is not a draft;
3. downloads the four release archives and calculates their SHA-256 checksums;
4. renders the formula template;
5. checks out `webcodr/homebrew-tap`, audits and installs the formula on Linux,
   and runs its Homebrew test; and
6. commits the updated `Formula/srchr.rb` with the separate-repository token.

The source repository needs a `HOMEBREW_TAP_TOKEN` Actions secret. It must be a
fine-grained token with Contents read/write permission on
`webcodr/homebrew-tap`; the workflow's repository-scoped `GITHUB_TOKEN` cannot
push to that repository.

## User Installation

Users install directly from the tap with:

```sh
brew install webcodr/tap/srchr
```

## Verification

The generated formula is checked with `brew audit --strict --online`, installed
on Linux, and tested with `brew test Formula/srchr.rb` before it is pushed. The
first release should also be manually installed on macOS Intel and Apple
Silicon.
