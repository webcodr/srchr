# Prefill Query Parameter Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `-q`/`--query` flag that prefills the interactive query and runs the search immediately on startup.

**Architecture:** Replace the hand-rolled `env::args().nth(1)` parsing in `main.rs` with a clap derive `Cli` struct (positional `path`, optional `--query`). Add `App::with_query` so the app can start with a seeded query. In `run()`, seed the query and, when non-empty, pre-arm the debounce so the first event-loop iteration launches a search.

**Tech Stack:** Rust, clap 4 (derive), ratatui/crossterm (existing TUI), existing `search`/`preview`/`editor` modules.

---

### Task 1: Add `App::with_query` constructor

**Files:**
- Modify: `rust/src/app.rs:11-19` (impl `App`)
- Test: `rust/src/app.rs` (existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing test**

Add these two tests inside `mod tests` in `rust/src/app.rs` (after the existing `typing_and_backspace_edit_query` test):

```rust
    #[test]
    fn with_query_seeds_the_query() {
        let app = App::with_query("foo".to_string());
        assert_eq!(app.query, "foo");
    }

    #[test]
    fn new_starts_with_empty_query() {
        let app = App::new();
        assert_eq!(app.query, "");
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml with_query_seeds_the_query`
Expected: FAIL to compile with "no function or associated item named `with_query`".

- [ ] **Step 3: Write minimal implementation**

Replace the `new` method in `rust/src/app.rs` (lines 12-19) with `with_query` plus a delegating `new`:

```rust
    pub fn new() -> Self {
        App::with_query(String::new())
    }

    pub fn with_query(query: String) -> Self {
        App {
            query,
            results: Vec::new(),
            selected: 0,
            status: String::new(),
        }
    }
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml --lib`
Expected: PASS, including `with_query_seeds_the_query` and `new_starts_with_empty_query`.

- [ ] **Step 5: Commit**

```bash
git add rust/src/app.rs
git commit -m "feat: add App::with_query constructor"
```

---

### Task 2: Add clap dependency and `Cli` struct

**Files:**
- Modify: `rust/Cargo.toml:14-22` (`[dependencies]`)
- Modify: `rust/src/main.rs:1-47` (imports, add `Cli`, rewrite `main`)
- Test: `rust/src/main.rs` (existing `#[cfg(test)] mod tests`)

- [ ] **Step 1: Add the clap dependency**

Add this line to the `[dependencies]` table in `rust/Cargo.toml` (after the `once_cell` line):

```toml
clap = { version = "4", features = ["derive"] }
```

- [ ] **Step 2: Write the failing test**

Add this test inside `mod tests` in `rust/src/main.rs` (after the existing tests):

```rust
    #[test]
    fn cli_definition_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn cli_defaults_path_to_dot_and_no_query() {
        use clap::Parser;
        let cli = Cli::parse_from(["srchr"]);
        assert_eq!(cli.path, PathBuf::from("."));
        assert_eq!(cli.query, None);
    }

    #[test]
    fn cli_parses_path_and_query() {
        use clap::Parser;
        let cli = Cli::parse_from(["srchr", "src", "-q", "fn"]);
        assert_eq!(cli.path, PathBuf::from("src"));
        assert_eq!(cli.query.as_deref(), Some("fn"));
    }
```

- [ ] **Step 3: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml cli_definition_is_valid`
Expected: FAIL to compile with "cannot find type `Cli`".

- [ ] **Step 4: Add the `Cli` struct and wire up `main`**

In `rust/src/main.rs`, add the clap import near the other `use` lines (after line 15):

```rust
use clap::Parser;
```

Add the `Cli` struct definition immediately above `fn main()` (before line 32):

```rust
/// Live-grep file search with fuzzy selection and syntax preview.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Cli {
    /// Directory to search (defaults to the current directory).
    #[arg(default_value = ".")]
    path: PathBuf,

    /// Prefill the search query and run it immediately on startup.
    #[arg(short, long)]
    query: Option<String>,
}
```

Replace the body of `fn main()` (lines 33-46) so it parses via clap. Keep calling `run` with only `path` for now (Task 3 updates `run` to take the query); this keeps the crate compiling after this task:

```rust
fn main() {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("srchr: not a terminal (this is an interactive tool)");
        std::process::exit(2);
    }

    let cli = Cli::parse();

    if let Err(e) = run(cli.path) {
        eprintln!("srchr: {e}");
        std::process::exit(1);
    }
}
```

Note: `cli.query` is intentionally unused in this task. Prefix it in the struct is not needed; clap keeps the field. If clippy flags an unused field here, ignore it — Task 3 consumes `cli.query`. (Run clippy only in Task 4, after Task 3.)

- [ ] **Step 5: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: The three new `cli_*` tests PASS and the crate compiles (`run` still takes one argument).

- [ ] **Step 6: Commit**

```bash
git add rust/Cargo.toml rust/Cargo.lock rust/src/main.rs
git commit -m "feat: parse args with clap and add --query flag"
```

---

### Task 3: Seed the query and search immediately in `run`

**Files:**
- Modify: `rust/src/main.rs:49-64` (`run` signature and app setup)

- [ ] **Step 1: Update the `main` call site**

In `rust/src/main.rs`, change the call in `main` from:

```rust
    if let Err(e) = run(cli.path) {
```

to:

```rust
    if let Err(e) = run(cli.path, cli.query) {
```

- [ ] **Step 2: Update the `run` signature and seed the app**

In `rust/src/main.rs`, change the `run` function signature from:

```rust
fn run(root: PathBuf) -> io::Result<()> {
```

to:

```rust
fn run(root: PathBuf, initial_query: Option<String>) -> io::Result<()> {
```

Then replace the app construction and the pending-query initialization. Change:

```rust
    let mut app = App::new();
    let (result_tx, result_rx): (Sender<SearchResult>, Receiver<SearchResult>) = mpsc::channel();
    let mut pending_query: Option<String> = None;
    let mut pending_at = Instant::now();
```

to:

```rust
    let seed = initial_query.unwrap_or_default();
    let mut app = App::with_query(seed.clone());
    let (result_tx, result_rx): (Sender<SearchResult>, Receiver<SearchResult>) = mpsc::channel();
    let mut pending_query: Option<String> = None;
    let mut pending_at = Instant::now();

    if !seed.is_empty() {
        pending_query = Some(seed);
        pending_at = Instant::now() - DEBOUNCE;
        app.status = "searching...".to_string();
    }
```

- [ ] **Step 3: Verify the project builds and all tests pass**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: PASS, all tests including Task 1 and Task 2 additions. No `run` arity errors.

- [ ] **Step 4: Commit**

```bash
git add rust/src/main.rs
git commit -m "feat: prefill and run search on startup when --query is given"
```

---

### Task 4: Verification gates and manual smoke test

**Files:** none (verification only)

- [ ] **Step 1: Format check**

Run: `cargo fmt --manifest-path rust/Cargo.toml -- --check`
Expected: no output (clean). If it fails, run `cargo fmt --manifest-path rust/Cargo.toml` and re-commit.

- [ ] **Step 2: Clippy**

Run: `cargo clippy --manifest-path rust/Cargo.toml -- -D warnings`
Expected: no warnings/errors.

- [ ] **Step 3: Full test run**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: all tests PASS.

- [ ] **Step 4: Ask the user for a manual TUI smoke test**

The interactive TUI needs a TTY and cannot be exercised headlessly (per `AGENTS.md`). Ask the user to run:

```sh
cargo run --manifest-path rust/Cargo.toml -- . -q TODO
```

Confirm with the user that: the TUI opens with `TODO` shown in the query input, results are already populated (or a valid empty/status state), the prefilled query is editable via backspace/typing, and `--help` (`cargo run --manifest-path rust/Cargo.toml -- --help`) shows the `-q/--query` and path arguments.

---

## Notes

- `Cargo.lock` should be committed alongside `Cargo.toml` in Task 2 since adding clap changes it.
- `-q ""` yields an empty seed, so `seed.is_empty()` is true and no startup search runs — matching the spec edge case.
