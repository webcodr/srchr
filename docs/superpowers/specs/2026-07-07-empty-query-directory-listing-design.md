# Empty Query Shows Directory Contents

## Problem

Today, when the search query is empty, `srchr` shows nothing:

- At startup with no `-q` flag (or `-q ""`), no search is scheduled and the
  result list stays empty (`main.rs`, `run()`, seed-empty branch).
- If the user types a query and then backspaces it down to empty,
  `Action::QueryChanged` clears results to `Vec::new()` and blanks the status
  line instead of running anything (`main.rs`, `QueryChanged` handling).

This means a user who launches `srchr` and hasn't typed anything yet sees a
blank screen with no sense of what's in the directory. An empty query should
instead browse the whole tree, exactly like `ls -R` combined with gitignore
awareness — reusing the same walk that content/name search already performs.

This spec also fixes a related cosmetic issue: paths produced when the search
root is `.` include a `./` prefix (e.g. `./src/main.rs`), which is not
useful in the result list and will be stripped for all rows, not just the new
directory listing.

## Relationship to prior specs

`docs/superpowers/specs/2026-07-07-prefill-query-design.md` states: *"`-q ""`
... behaves identically to no `-q`: empty query, no startup search, empty
status."* This spec supersedes that specific claim: an empty query (via `-q
""` or no `-q` at all) now triggers a directory listing at startup, just like
backspacing to empty triggers one during interactive use. The "identical
behavior between `-q ""` and no `-q`" invariant itself still holds — both now
schedule the same listing.

## Design

### 1. `search.rs`: new `list_dir` function

```rust
/// Walk `root` (gitignore-aware), producing one FileHit per file with no
/// query applied — used when the search query is empty to browse the whole
/// tree. Returns empty if `cancel` is set. Cancellation is checked per entry.
pub fn list_dir(root: &Path, cancel: &Arc<AtomicBool>) -> Vec<FileHit>
```

- Uses the same `WalkBuilder::new(root).require_git(false)` setup as
  `search()`, with the same per-entry cancellation check and the same
  "skip unreadable entries silently" behavior.
- Skips name/content matching entirely. Every file becomes:
  ```rust
  FileHit { path: normalize_path(root, path), match_count: 0, first_line: None }
  ```
- Calls `sort_hits` before returning. Since every entry is a name-only tie
  (`match_count: 0`, `first_line: None`), `sort_hits`'s tie-break by path
  naturally yields alphabetical-by-path ordering — matching the existing
  `[name]`-row rendering with no changes needed in `ui.rs`.

### 2. Path normalization helper (used by both `search()` and `list_dir()`)

```rust
/// Strip a leading "./" path component so rows read "src/main.rs" instead of
/// "./src/main.rs" when the root is ".". Paths built from other roots are
/// unaffected (they never gain this prefix from WalkBuilder).
fn normalize_path(path: &Path) -> PathBuf {
    path.strip_prefix(".").unwrap_or(path).to_path_buf()
}
```

`search()` will apply this when constructing each `FileHit` (currently
`path.to_path_buf()` at `search.rs:113`), and `list_dir()` will apply it the
same way. No changes needed to `editor.rs`'s existing `+`/`-` guard, since
that guard already rewrites relative paths defensively regardless of `./`
prefix.

### 3. `main.rs`: unify empty and non-empty query handling

Both call sites that currently special-case "query is empty" will instead
always schedule a background job — the job itself picks `list_dir` vs.
`search` based on whether the query string is empty.

**`spawn_search`** (`main.rs:227`) gains a branch:

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

**Startup** (`main.rs:68-78`): remove the `if !seed.is_empty()` guard so a job
is always scheduled, empty seed or not:

```rust
let seed = initial_query.unwrap_or_default();
let mut app = App::with_query(seed.clone());
...
pending_query = Some(seed);
pending_at = Instant::now() - DEBOUNCE;
app.status = "searching...".to_string();
```

**`Action::QueryChanged`** (`main.rs:126-139`): remove the
`if app.query.is_empty() { ... } else { ... }` split; always schedule the
debounced job the same way:

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

The existing debounce loop (`main.rs:96-104`) and result-application logic
(`main.rs:86-94`) are unchanged — they already handle arbitrary query
strings, including empty ones, and already set status to
`"searching..."` while in flight and `"{n} files"` once results land. No new
status text is introduced.

### 4. No changes to `app.rs` or `ui.rs`

`FileHit` rows with `first_line: None` already render as `[name]` regardless
of how they were produced. `App` has no knowledge of which function produced
its `results`.

## Testing

- `search.rs`: unit tests for `list_dir`
  - returns one `FileHit` per file in a temp directory tree, `match_count: 0`,
    `first_line: None`.
  - respects `.gitignore` (ignored files excluded).
  - returns `Vec::new()` when `cancel` is already set.
  - `normalize_path` strips a leading `./` component; leaves other paths
    unchanged.
- `main.rs`: extend/adjust existing tests around `QueryChanged` handling and
  startup scheduling to confirm:
  - an empty seed at startup still results in `pending_query = Some("")`
    (schedules a job) rather than `None`.
  - backspacing a query to empty produces `Action::QueryChanged` and (via the
    unified handling) schedules a job rather than synchronously clearing
    results.
- Existing `search()` tests continue to pass with `normalize_path` applied
  (paths in those tests use temp dirs, not `.`, so `./` stripping is a no-op
  for them — verify no regressions).

## Non-goals

- No shallow/`ls`-style single-level listing — the directory listing is fully
  recursive, matching the existing `search()` walk.
- No new status text for the listing state; it reuses `"searching..."` and
  `"{n} files"`.
- No change to sort behavior beyond what `sort_hits` already does for
  name-only ties (alphabetical by path).
