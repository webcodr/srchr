# Empty Query Shows Directory Contents Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** When the search query is empty, `srchr` lists all files under the search root (recursive, gitignore-aware) instead of showing a blank result list, and paths no longer show a leading `./` prefix.

**Architecture:** Add a `list_dir` function to `search.rs` that walks the tree the same way `search()` does but without any name/content matching, producing name-only `FileHit` rows. Add a shared `normalize_path` helper to strip a leading `./` path component, used by both `search()` and `list_dir()`. Wire `main.rs` so the empty-query case runs through the same debounce/spawn machinery as any other query, picking `list_dir` vs. `search` based on whether the query string is empty.

**Tech Stack:** Rust, `ignore` crate (gitignore-aware walking), existing `srchr::search` module.

**Spec:** `docs/superpowers/specs/2026-07-07-empty-query-directory-listing-design.md`

---

### Task 1: Add `normalize_path` helper with tests

**Files:**
- Modify: `rust/src/search.rs`

- [ ] **Step 1: Write the failing tests**

Add to the `mod tests` block in `rust/src/search.rs` (after the existing `filename_match_is_smart_case` test, before `ordering_content_before_name_only_then_by_count`):

```rust
    #[test]
    fn normalize_path_strips_leading_dot_slash() {
        assert_eq!(
            normalize_path(Path::new("./src/main.rs")),
            PathBuf::from("src/main.rs")
        );
    }

    #[test]
    fn normalize_path_leaves_other_paths_unchanged() {
        assert_eq!(
            normalize_path(Path::new("src/main.rs")),
            PathBuf::from("src/main.rs")
        );
        assert_eq!(
            normalize_path(Path::new("/tmp/foo/bar.rs")),
            PathBuf::from("/tmp/foo/bar.rs")
        );
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path rust/Cargo.toml normalize_path`
Expected: FAIL with "cannot find function `normalize_path` in this scope"

- [ ] **Step 3: Implement `normalize_path`**

Add this function to `rust/src/search.rs`, directly above `sort_hits` (before line 78, `/// Content matches first...`):

```rust
/// Strip a leading "./" path component so rows read "src/main.rs" instead of
/// "./src/main.rs" when the search root is ".". Paths built from other roots
/// are unaffected, since they never gain this prefix from `WalkBuilder`.
fn normalize_path(path: &Path) -> PathBuf {
    path.strip_prefix(".").unwrap_or(path).to_path_buf()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml normalize_path`
Expected: PASS (2 tests)

- [ ] **Step 5: Commit**

```bash
cd /home/dh/projects/srchr
git add rust/src/search.rs
git commit -m "feat: add normalize_path helper to strip leading ./ from paths"
```

---

### Task 2: Apply `normalize_path` in `search()`

**Files:**
- Modify: `rust/src/search.rs:112-116`
- Test: `rust/tests/search_tests.rs`

- [ ] **Step 1: Write the failing test**

Add to `rust/tests/search_tests.rs` (after `merges_name_and_content_hits_deduped`):

```rust
#[test]
fn search_strips_leading_dot_slash_from_root() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "alpha.rs", "alpha token\n");
    let cwd = std::env::current_dir().unwrap();
    std::env::set_current_dir(dir.path()).unwrap();
    let q = Query::compile("alpha").unwrap();
    let hits = search(&q, Path::new("."), &cancel_never());
    std::env::set_current_dir(cwd).unwrap();

    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].path, std::path::PathBuf::from("alpha.rs"));
}
```

Note: this test changes the process's current directory temporarily. It must
not run concurrently with other tests that depend on `std::env::current_dir`.
No other test in this suite reads `current_dir`, so this is safe as written.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml search_strips_leading_dot_slash_from_root`
Expected: FAIL — `hits[0].path` is `./alpha.rs`, not `alpha.rs`

- [ ] **Step 3: Apply `normalize_path` in `search()`**

In `rust/src/search.rs`, change the `FileHit` construction inside `search()`
(currently at lines 112-116):

```rust
        if count > 0 || name_hit {
            hits.push(FileHit {
                path: path.to_path_buf(),
                match_count: count,
                first_line: first,
            });
        }
```

to:

```rust
        if count > 0 || name_hit {
            hits.push(FileHit {
                path: normalize_path(path),
                match_count: count,
                first_line: first,
            });
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml --test search_tests`
Expected: PASS (all tests in `search_tests.rs`, including the new one and the
pre-existing `merges_name_and_content_hits_deduped`, `respects_gitignore`,
`cancel_flag_returns_empty`)

- [ ] **Step 5: Commit**

```bash
cd /home/dh/projects/srchr
git add rust/src/search.rs rust/tests/search_tests.rs
git commit -m "feat: strip leading ./ from search() result paths"
```

---

### Task 3: Add `list_dir` function with tests

**Files:**
- Modify: `rust/src/search.rs`
- Test: `rust/tests/search_tests.rs`

- [ ] **Step 1: Write the failing tests**

Add to `rust/tests/search_tests.rs`, update the import line at the top from:

```rust
use srchr::search::{search, Query};
```

to:

```rust
use srchr::search::{list_dir, search, Query};
```

Then add these tests at the end of the file:

```rust
#[test]
fn list_dir_returns_all_files_as_name_only_hits() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rs", "hello\n");
    write(dir.path(), "b.txt", "world\n");
    let hits = list_dir(dir.path(), &cancel_never());

    let names: Vec<_> = hits
        .iter()
        .map(|h| h.path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"a.rs".to_string()));
    assert!(names.contains(&"b.txt".to_string()));
    for h in &hits {
        assert_eq!(h.match_count, 0);
        assert_eq!(h.first_line, None);
    }
}

#[test]
fn list_dir_respects_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".gitignore", "ignored/\n");
    write(dir.path(), "ignored/secret.rs", "x\n");
    write(dir.path(), "kept.rs", "x\n");
    let hits = list_dir(dir.path(), &cancel_never());
    let names: Vec<_> = hits
        .iter()
        .map(|h| h.path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"kept.rs".to_string()));
    assert!(!names.contains(&"secret.rs".to_string()));
}

#[test]
fn list_dir_cancel_flag_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rs", "hello\n");
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let hits = list_dir(dir.path(), &cancel);
    assert!(hits.is_empty());
}

#[test]
fn list_dir_sorts_alphabetically_by_path() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "z.rs", "x\n");
    write(dir.path(), "a.rs", "x\n");
    write(dir.path(), "m.rs", "x\n");
    let hits = list_dir(dir.path(), &cancel_never());
    let names: Vec<_> = hits
        .iter()
        .map(|h| h.path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    assert_eq!(names, vec!["a.rs", "m.rs", "z.rs"]);
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test --manifest-path rust/Cargo.toml --test search_tests list_dir`
Expected: FAIL with "cannot find function `list_dir`" / unresolved import

- [ ] **Step 3: Implement `list_dir`**

Add this function to `rust/src/search.rs`, directly below the `search()`
function (after its closing brace, before the `#[cfg(test)]` block):

```rust
/// Walk `root` (gitignore-aware), producing one FileHit per file with no
/// query applied. Used when the search query is empty, to browse the whole
/// tree. Returns empty if `cancel` is set. Cancellation is checked per entry.
pub fn list_dir(root: &Path, cancel: &Arc<AtomicBool>) -> Vec<FileHit> {
    let mut hits: Vec<FileHit> = Vec::new();

    for result in WalkBuilder::new(root).require_git(false).build() {
        if cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue, // skip unreadable entries silently
        };
        if !entry.file_type().is_some_and(|ft| ft.is_file()) {
            continue;
        }
        hits.push(FileHit {
            path: normalize_path(entry.path()),
            match_count: 0,
            first_line: None,
        });
    }

    sort_hits(&mut hits);
    hits
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml --test search_tests`
Expected: PASS (all tests in `search_tests.rs`)

- [ ] **Step 5: Commit**

```bash
cd /home/dh/projects/srchr
git add rust/src/search.rs rust/tests/search_tests.rs
git commit -m "feat: add list_dir to browse the whole tree with no query"
```

---

### Task 4: Wire `spawn_search` to use `list_dir` for empty queries

**Files:**
- Modify: `rust/src/main.rs:22` (import), `rust/src/main.rs:227-240` (`spawn_search`)

- [ ] **Step 1: Write the failing test**

Add to the `mod tests` block in `rust/src/main.rs` (after `cli_parses_path_and_query`, at the end of the file):

```rust
    #[test]
    fn spawn_search_with_empty_query_lists_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("a.rs"), "hello").unwrap();
        std::fs::write(dir.path().join("b.txt"), "world").unwrap();

        let (tx, rx) = mpsc::channel();
        let _cancel = spawn_search("", dir.path(), tx);
        let res = rx.recv_timeout(Duration::from_secs(2)).unwrap();

        assert_eq!(res.query, "");
        assert!(res.error.is_none());
        let names: Vec<_> = res
            .hits
            .iter()
            .map(|h| h.path.file_name().unwrap().to_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"a.rs".to_string()));
        assert!(names.contains(&"b.txt".to_string()));
        for h in &res.hits {
            assert_eq!(h.match_count, 0);
            assert_eq!(h.first_line, None);
        }
    }
```

This requires `tempfile` as a dev-dependency, which is already declared in
`rust/Cargo.toml`'s `[dev-dependencies]` (shared across the binary and its
tests). No new imports are needed inside `mod tests`: it already starts with
`use super::*;`, which brings in `Duration` and `mpsc` from the top of
`main.rs` since child modules can see private items of their parent module.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml spawn_search_with_empty_query_lists_directory`
Expected: FAIL — with the current implementation, `Query::compile("")`
succeeds (empty pattern matches everything) and `search()` runs instead of
`list_dir()`, so `match_count` will be nonzero and `first_line` will be
`Some(_)` for at least one file, failing the `assert_eq!(h.match_count, 0)`
assertion.

- [ ] **Step 3: Update `spawn_search` to branch on empty query**

In `rust/src/main.rs`, update the import at line 22 from:

```rust
use srchr::search::{search, FileHit, Query};
```

to:

```rust
use srchr::search::{list_dir, search, FileHit, Query};
```

Then change `spawn_search` (currently lines 227-240) from:

```rust
fn spawn_search(query: &str, root: &Path, tx: Sender<SearchResult>) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let query = query.to_string();
    let root = root.to_path_buf();
    thread::spawn(move || {
        let (hits, error) = match Query::compile(&query) {
            Ok(q) => (search(&q, &root, &worker_cancel), None),
            Err(e) => (Vec::new(), Some(e)),
        };
        let _ = tx.send(SearchResult { query, hits, error });
    });
    cancel
}
```

to:

```rust
fn spawn_search(query: &str, root: &Path, tx: Sender<SearchResult>) -> Arc<AtomicBool> {
    let cancel = Arc::new(AtomicBool::new(false));
    let worker_cancel = Arc::clone(&cancel);
    let query = query.to_string();
    let root = root.to_path_buf();
    thread::spawn(move || {
        let (hits, error) = if query.is_empty() {
            (list_dir(&root, &worker_cancel), None)
        } else {
            match Query::compile(&query) {
                Ok(q) => (search(&q, &root, &worker_cancel), None),
                Err(e) => (Vec::new(), Some(e)),
            }
        };
        let _ = tx.send(SearchResult { query, hits, error });
    });
    cancel
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: PASS (all tests, including
`spawn_search_with_empty_query_lists_directory`)

- [ ] **Step 5: Commit**

```bash
cd /home/dh/projects/srchr
git add rust/src/main.rs
git commit -m "feat: spawn_search lists directory contents for empty query"
```

---

### Task 5: Schedule a job at startup even for an empty seed query

**Files:**
- Modify: `rust/src/main.rs:68-78`

- [ ] **Step 1: Update the startup scheduling code**

In `rust/src/main.rs`, change (currently lines 68-78):

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

to:

```rust
    let seed = initial_query.unwrap_or_default();
    let mut app = App::with_query(seed.clone());
    let (result_tx, result_rx): (Sender<SearchResult>, Receiver<SearchResult>) = mpsc::channel();
    let mut pending_query: Option<String> = Some(seed);
    let mut pending_at = Instant::now() - DEBOUNCE;
    app.status = "searching...".to_string();
```

This removes the `if !seed.is_empty()` guard: a background job (search or
directory listing, decided inside `spawn_search`) is now always scheduled to
run immediately on startup (the `Instant::now() - DEBOUNCE` backdating makes
the very first debounce-loop iteration fire it right away, same as before for
non-empty seeds).

There is no separate unit test for this block, since `run()` drives a live
terminal event loop and isn't unit-tested elsewhere in this codebase (see
`AGENTS.md`: the interactive TUI needs a TTY and agents cannot fully test
it). Verification here is: (a) the project builds and all existing tests
still pass, and (b) the manual smoke test in Task 7 below.

- [ ] **Step 2: Verify the project builds**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: builds with no errors

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: PASS (no regressions)

- [ ] **Step 4: Commit**

```bash
cd /home/dh/projects/srchr
git add rust/src/main.rs
git commit -m "feat: always schedule a startup job, even for an empty seed query"
```

---

### Task 6: Schedule a debounced job on `QueryChanged`, even when the query becomes empty

**Files:**
- Modify: `rust/src/main.rs:126-139`

- [ ] **Step 1: Update the `QueryChanged` handling**

In `rust/src/main.rs`, change (currently lines 126-139):

```rust
                        Action::QueryChanged => {
                            if let Some(cancel) = current_cancel.take() {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            if app.query.is_empty() {
                                pending_query = None;
                                app.set_results(Vec::new());
                                app.status.clear();
                            } else {
                                pending_query = Some(app.query.clone());
                                pending_at = Instant::now();
                                app.status = "searching...".to_string();
                            }
                        }
```

to:

```rust
                        Action::QueryChanged => {
                            if let Some(cancel) = current_cancel.take() {
                                cancel.store(true, Ordering::Relaxed);
                            }
                            pending_query = Some(app.query.clone());
                            pending_at = Instant::now();
                            app.status = "searching...".to_string();
                        }
```

This removes the special case that synchronously cleared results when the
query became empty. Now backspacing to empty schedules a debounced background
job the same way any other query edit does; `spawn_search` (Task 4) will run
`list_dir` for it once the debounce elapses.

As with Task 5, this branch lives inside `run()`'s event loop and has no
existing unit test harness in this codebase; verification is via the build,
full test suite, and manual smoke test (Task 7).

- [ ] **Step 2: Verify the project builds**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: builds with no errors

- [ ] **Step 3: Run the full test suite**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: PASS (no regressions)

- [ ] **Step 4: Commit**

```bash
cd /home/dh/projects/srchr
git add rust/src/main.rs
git commit -m "feat: schedule directory listing when query is backspaced to empty"
```

---

### Task 7: Full verification and manual smoke test

**Files:** none (verification only)

- [ ] **Step 1: Run fmt, clippy, and full test suite**

```bash
cargo fmt --manifest-path rust/Cargo.toml -- --check
cargo clippy --manifest-path rust/Cargo.toml -- -D warnings
cargo test --manifest-path rust/Cargo.toml
```

Expected: all three succeed with no diffs, warnings, or failures. If `fmt`
reports a diff, run `cargo fmt --manifest-path rust/Cargo.toml` (without
`--check`) and re-verify, then amend the affected commit(s) or fold the
formatting fix into a new small commit.

- [ ] **Step 2: Manual smoke test (requires a human with a TTY)**

Per `AGENTS.md`, the interactive TUI needs a TTY and cannot be fully tested
by an agent. Ask the user to run:

```bash
cargo run --manifest-path rust/Cargo.toml -- .
```

And confirm:
- On launch, with no query typed, the result list immediately shows files
  from the current directory tree (not blank), with `[name]` rows, no
  `./` prefix on any path, and status reads `"N files"` once loaded.
- Typing a query still filters/searches as before.
- Backspacing the query all the way to empty brings back the full directory
  listing (after the short debounce), not a blank list.
- `srchr -q ""` behaves the same as `srchr` with no `-q` flag: directory
  listing shown on startup.
- Selecting a listed file and pressing Enter still opens it correctly in
  `$EDITOR`.

- [ ] **Step 3: No commit needed for this task** (verification only; if the
  smoke test surfaces an issue, fix it in a new commit and re-run this task).

---

## Self-Review Notes

- **Spec coverage:** `list_dir` (Task 3), `normalize_path` (Tasks 1–2),
  `spawn_search` branching (Task 4), startup scheduling (Task 5),
  `QueryChanged` scheduling (Task 6), status text reuse (covered implicitly —
  no new status strings introduced, existing `"searching..."` / `"{n}
  files"` logic in `run()`'s result-application block is untouched), no
  changes to `app.rs`/`ui.rs` (confirmed, no task touches them), manual smoke
  test (Task 7) per `AGENTS.md` requirement. All spec sections are covered.
- **Placeholder scan:** no TBD/TODO markers; every step has literal code or
  exact commands.
- **Type consistency:** `list_dir(root: &Path, cancel: &Arc<AtomicBool>) ->
  Vec<FileHit>` matches its use in Task 4's `spawn_search` (`list_dir(&root,
  &worker_cancel)`) and its test call sites (`list_dir(dir.path(),
  &cancel_never())`). `normalize_path(path: &Path) -> PathBuf` matches its use
  in both `search()` and `list_dir()`.
