# GitHub Release Build Workflow Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a manual GitHub Actions release workflow that creates a draft release, builds six `srchr` binaries, uploads raw artifacts, and attaches `.tar.gz` archives to the release.

**Architecture:** Keep the existing CI workflow unchanged and add one focused release workflow at `.github/workflows/build.yml`. The workflow has one release-creation job and one matrix build job adapted from `webcodr/server-runner`, with commands run from the `rust/` crate directory.

**Tech Stack:** GitHub Actions YAML, `softprops/action-gh-release@v2`, `actions/checkout@v4`, `actions/upload-artifact@v4`, `dtolnay/rust-toolchain@stable`, Rust/Cargo.

---

## File Structure

- Create: `.github/workflows/build.yml` - manual release workflow for creating the draft GitHub release, building target binaries, uploading raw artifacts, compressing binaries, and attaching compressed assets.
- Keep unchanged: `.github/workflows/rust.yml` - existing push/PR Rust fmt, clippy, and test checks.
- Keep unchanged: `rust/Cargo.toml` - package metadata and binary name already define `srchr`.

## Task 1: Add Release Workflow

**Files:**
- Create: `.github/workflows/build.yml`

- [ ] **Step 1: Create workflow file**

Create `.github/workflows/build.yml` with this exact content:

```yaml
name: Build and Release

on:
  workflow_dispatch:
    inputs:
      version:
        description: Version
        required: true
        default: 0.1.0

permissions:
  contents: write

jobs:
  create_release:
    runs-on: ubuntu-latest
    steps:
      - name: Create draft release
        uses: softprops/action-gh-release@v2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ inputs.version }}
          name: ${{ inputs.version }}
          target_commitish: ${{ github.sha }}
          draft: true
          prerelease: false

  build:
    runs-on: ${{ matrix.runner }}
    needs: create_release
    strategy:
      matrix:
        include:
          - name: macos-aarch64
            runner: macos-latest
            target: aarch64-apple-darwin
            artifact: srchr
          - name: macos-x86-64
            runner: macos-latest
            target: x86_64-apple-darwin
            artifact: srchr
          - name: linux-aarch64
            runner: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            artifact: srchr
          - name: linux-x86-64
            runner: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            artifact: srchr
          - name: windows-aarch64
            runner: windows-latest
            target: aarch64-pc-windows-msvc
            artifact: srchr.exe
          - name: windows-x86-64
            runner: windows-latest
            target: x86_64-pc-windows-msvc
            artifact: srchr.exe
    defaults:
      run:
        working-directory: rust
    steps:
      - name: Checkout code
        uses: actions/checkout@v4

      - name: Set up Rust
        uses: dtolnay/rust-toolchain@stable

      - name: Install Linux aarch64 linker
        if: ${{ matrix.name == 'linux-aarch64' }}
        run: sudo apt-get update && sudo apt-get install -y gcc-aarch64-linux-gnu

      - name: Configure Linux aarch64 linker
        if: ${{ matrix.name == 'linux-aarch64' }}
        run: |
          mkdir -p .cargo
          printf '[target.aarch64-unknown-linux-gnu]\nlinker = "aarch64-linux-gnu-gcc"\n' > .cargo/config.toml

      - name: Install build target
        run: rustup target add ${{ matrix.target }}

      - name: Build
        run: cargo build --release --target=${{ matrix.target }}

      - name: Upload raw artifact
        uses: actions/upload-artifact@v4
        with:
          name: srchr-${{ inputs.version }}-${{ matrix.name }}
          path: rust/target/${{ matrix.target }}/release/${{ matrix.artifact }}

      - name: Compress artifact
        run: tar -czf ../srchr-${{ inputs.version }}-${{ matrix.name }}.tar.gz -C target/${{ matrix.target }}/release ${{ matrix.artifact }}

      - name: Upload release asset
        uses: softprops/action-gh-release@v2
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        with:
          tag_name: ${{ inputs.version }}
          draft: true
          files: srchr-${{ inputs.version }}-${{ matrix.name }}.tar.gz
```

- [ ] **Step 2: Verify workflow syntax by inspection**

Check these facts in `.github/workflows/build.yml`:

```text
The workflow name is Build and Release.
The only trigger is workflow_dispatch.
The workflow has a required version input.
permissions.contents is write.
create_release uses softprops/action-gh-release@v2 with draft: true.
tag_name and name both use ${{ inputs.version }} with no v prefix.
target_commitish uses ${{ github.sha }}.
The build matrix contains exactly six include entries.
The build job defaults to working-directory: rust.
Upload raw artifact path starts with rust/target because upload-artifact does not use shell working-directory defaults.
The compressed archive is written one directory above rust/ so release upload can read it from the repository root.
The release asset upload step includes draft: true.
```

- [ ] **Step 3: Run existing Rust verification**

Run from workspace root:

```bash
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

Expected: all three commands exit successfully. These checks do not exercise the GitHub workflow itself, but they verify that the new release job is not masking existing Rust failures.

- [ ] **Step 4: Review git diff**

Run:

```bash
git diff -- .github/workflows/build.yml docs/superpowers/specs/2026-07-07-github-release-build-workflow-design.md docs/superpowers/plans/2026-07-07-github-release-build-workflow.md
```

Expected: the diff shows only the new workflow file plus the design and plan documents for this change.

- [ ] **Step 5: Commit only if explicitly requested**

Do not commit by default. If the user explicitly requests a commit, inspect `git status`, `git diff`, and `git log --oneline -10`, then commit only the intended files with a concise message such as:

```bash
git add .github/workflows/build.yml docs/superpowers/specs/2026-07-07-github-release-build-workflow-design.md docs/superpowers/plans/2026-07-07-github-release-build-workflow.md
git commit -m "ci: add release build workflow"
```

## Self-Review

- Spec coverage: Task 1 covers manual trigger, exact version tag/name, draft release, target commit pinning, six macOS/Linux/Windows targets, raw artifact upload, `.tar.gz` release assets, Linux aarch64 linker setup, and no automatic release trigger.
- Placeholder scan: No placeholders remain; all file paths, matrix entries, commands, and expected checks are explicit.
- Type consistency: Matrix field names are consistently `name`, `runner`, `target`, and `artifact`; workflow references use `inputs.version` consistently.
