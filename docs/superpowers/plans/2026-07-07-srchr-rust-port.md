# srchr Rust Port Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `srchr.fish` / `srchr.sh` shell functions with a single self-contained Rust binary that embeds file+content search, an interactive live-grep TUI, and syntax-highlighted preview, depending on no external runtime tool except `$EDITOR`.

**Architecture:** A `ratatui`/`crossterm` TUI drives a live content+filename search built on ripgrep's library crates (`ignore`, `grep-regex`, `grep-searcher`, `regex`). Search runs on a debounced, cancellable background thread and streams file-level results (with match counts) to the UI. Preview is rendered with `syntect`. Selection launches `$EDITOR` at the first match.

**Tech Stack:** Rust 1.95, `ignore` 0.4, `grep-regex` 0.1, `grep-searcher` 0.1, `regex` 1, `ratatui` 0.29, `crossterm` 0.28, `syntect` 5.

**Design spec:** `docs/superpowers/specs/2026-07-07-srchr-rust-port-design.md`

---

## File Structure

```
srchr/
  srchr.fish, srchr.sh        # kept until parity confirmed, then retired (Task 13)
  rust/
    Cargo.toml
    src/
      main.rs        # arg parsing, TTY check, terminal setup/teardown, event loop wiring
      search.rs      # FileHit, smart-case, filename + content match, walk, aggregate
      app.rs         # TUI state: query, results, selection, status
      ui.rs          # ratatui rendering (input box, results list, preview pane)
      preview.rs     # line-range math + syntect styling into ratatui lines
      editor.rs      # $EDITOR resolution, path-safety guard, arg-vector construction
    tests/
      search_tests.rs   # integration tests over a fixture tree
```

Module responsibilities and boundaries:
- `search` — pure-ish search logic. Public: `is_case_sensitive`, `Query`, `FileHit`, `search()`. No UI knowledge.
- `editor` — pure functions: `resolve_editor`, `normalize_path`, `editor_args`, plus a `launch` that spawns the process.
- `preview` — pure `preview_start`, `PreviewData`, `build_preview`; styling helper used by `ui`.
- `app` — state machine, no rendering or blocking I/O.
- `ui` — pure rendering from `app` state.
- `main` — owns the terminal, the debounce timer, and the background search thread.

---

## Task 1: Project scaffold

**Files:**
- Create: `rust/Cargo.toml`
- Create: `rust/src/main.rs`

- [ ] **Step 1: Create `rust/Cargo.toml`**

```toml
[package]
name = "srchr"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "srchr"
path = "src/main.rs"

[dependencies]
ignore = "0.4"
grep-regex = "0.1"
grep-searcher = "0.1"
regex = "1"
ratatui = "0.29"
crossterm = "0.28"
syntect = "5"

[dev-dependencies]
tempfile = "3"
```

- [ ] **Step 2: Create minimal `rust/src/main.rs`**

```rust
fn main() {
    println!("srchr");
}
```

- [ ] **Step 3: Build to verify the toolchain and deps resolve**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: compiles successfully (downloads crates on first run).

- [ ] **Step 4: Commit**

```bash
git add rust/Cargo.toml rust/src/main.rs
git commit -m "chore: scaffold rust srchr crate"
```

---

## Task 2: Smart-case detection

**Files:**
- Create: `rust/src/search.rs`
- Modify: `rust/src/main.rs` (declare module)

- [ ] **Step 1: Write the failing test** — append to `rust/src/search.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_query_is_case_insensitive() {
        assert!(!is_case_sensitive("todo"));
    }

    #[test]
    fn uppercase_char_makes_it_case_sensitive() {
        assert!(is_case_sensitive("Todo"));
    }
}
```

- [ ] **Step 2: Add the module declaration** — at the top of `rust/src/main.rs`:

```rust
mod search;

fn main() {
    println!("srchr");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml is_case`
Expected: FAIL — `is_case_sensitive` not found.

- [ ] **Step 4: Implement** — add to the top of `rust/src/search.rs`:

```rust
/// Smart-case: case-sensitive only when the query contains an uppercase char.
pub fn is_case_sensitive(query: &str) -> bool {
    query.chars().any(|c| c.is_uppercase())
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml is_case`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/src/search.rs rust/src/main.rs
git commit -m "feat: smart-case detection"
```

---

## Task 3: FileHit type and Query compilation

**Files:**
- Modify: `rust/src/search.rs`

- [ ] **Step 1: Write the failing test** — add to the `tests` module in `rust/src/search.rs`:

```rust
    #[test]
    fn query_compiles_valid_pattern() {
        assert!(Query::compile("foo").is_ok());
    }

    #[test]
    fn query_rejects_invalid_regex() {
        assert!(Query::compile("foo(").is_err());
    }

    #[test]
    fn query_uppercase_is_case_sensitive() {
        let q = Query::compile("Foo").unwrap();
        assert!(q.case_sensitive);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml query_`
Expected: FAIL — `Query` not found.

- [ ] **Step 3: Implement** — add near the top of `rust/src/search.rs`:

```rust
use std::path::PathBuf;
use grep_regex::RegexMatcher;
use regex::RegexBuilder;

/// One result row: a file that matched by name and/or content.
#[derive(Debug, Clone)]
pub struct FileHit {
    pub path: PathBuf,
    /// Number of content matches; 0 for name-only hits.
    pub match_count: usize,
    /// First matching line (1-based); None for name-only hits.
    pub first_line: Option<usize>,
}

/// A compiled query: a content matcher (grep) and a filename matcher (regex).
pub struct Query {
    pub content: RegexMatcher,
    pub name: regex::Regex,
    pub case_sensitive: bool,
}

impl Query {
    pub fn compile(pattern: &str) -> Result<Query, String> {
        let case_sensitive = is_case_sensitive(pattern);
        let content = grep_regex::RegexMatcherBuilder::new()
            .case_insensitive(!case_sensitive)
            .build(pattern)
            .map_err(|e| e.to_string())?;
        let name = RegexBuilder::new(pattern)
            .case_insensitive(!case_sensitive)
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Query { content, name, case_sensitive })
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml query_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/search.rs
git commit -m "feat: Query compilation and FileHit type"
```

---

## Task 4: Per-file content search

**Files:**
- Modify: `rust/src/search.rs`

- [ ] **Step 1: Write the failing test** — add to the `tests` module:

```rust
    use std::io::Write;

    fn write_file(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    #[test]
    fn content_search_counts_and_first_line() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_file(dir.path(), "a.txt", "alpha\nbeta\nalpha\n");
        let q = Query::compile("alpha").unwrap();
        let (count, first) = search_file_content(&q, &p).unwrap();
        assert_eq!(count, 2);
        assert_eq!(first, Some(1));
    }

    #[test]
    fn content_search_no_match_is_zero() {
        let dir = tempfile::tempdir().unwrap();
        let p = write_file(dir.path(), "a.txt", "nothing here\n");
        let q = Query::compile("zzz").unwrap();
        let (count, first) = search_file_content(&q, &p).unwrap();
        assert_eq!(count, 0);
        assert_eq!(first, None);
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml content_search`
Expected: FAIL — `search_file_content` not found.

- [ ] **Step 3: Implement** — add to `rust/src/search.rs`:

```rust
use std::path::Path;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;

/// Returns (total match count, first matching line number 1-based).
pub fn search_file_content(query: &Query, path: &Path) -> std::io::Result<(usize, Option<usize>)> {
    let mut count = 0usize;
    let mut first: Option<usize> = None;
    Searcher::new().search_path(
        &query.content,
        path,
        UTF8(|lnum, _line| {
            count += 1;
            if first.is_none() {
                first = Some(lnum as usize);
            }
            Ok(true)
        }),
    )?;
    Ok((count, first))
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml content_search`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/search.rs
git commit -m "feat: per-file content search with count and first line"
```

---

## Task 5: Filename matching

**Files:**
- Modify: `rust/src/search.rs`

- [ ] **Step 1: Write the failing test** — add to the `tests` module:

```rust
    #[test]
    fn filename_match_uses_basename() {
        let q = Query::compile("config").unwrap();
        assert!(name_matches(&q, std::path::Path::new("src/config.rs")));
        assert!(!name_matches(&q, std::path::Path::new("src/main.rs")));
    }

    #[test]
    fn filename_match_is_smart_case() {
        let q = Query::compile("readme").unwrap();
        assert!(name_matches(&q, std::path::Path::new("README.md")));
        let q2 = Query::compile("README").unwrap();
        assert!(!name_matches(&q2, std::path::Path::new("readme.md")));
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml filename_match`
Expected: FAIL — `name_matches` not found.

- [ ] **Step 3: Implement** — add to `rust/src/search.rs` (matches `fd`'s default of testing the basename):

```rust
/// True if the file's basename matches the query (mirrors `fd` default).
pub fn name_matches(query: &Query, path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => query.name.is_match(name),
        None => false,
    }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml filename_match`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/search.rs
git commit -m "feat: smart-case filename matching on basename"
```

---

## Task 6: Result ordering

**Files:**
- Modify: `rust/src/search.rs`

- [ ] **Step 1: Write the failing test** — add to the `tests` module:

```rust
    #[test]
    fn ordering_content_before_name_only_then_by_count() {
        let mut hits = vec![
            FileHit { path: "z_name.rs".into(), match_count: 0, first_line: None },
            FileHit { path: "b.rs".into(), match_count: 2, first_line: Some(1) },
            FileHit { path: "a.rs".into(), match_count: 5, first_line: Some(3) },
        ];
        sort_hits(&mut hits);
        let paths: Vec<_> = hits.iter().map(|h| h.path.to_str().unwrap()).collect();
        assert_eq!(paths, vec!["a.rs", "b.rs", "z_name.rs"]);
    }

    #[test]
    fn ordering_breaks_count_ties_by_path() {
        let mut hits = vec![
            FileHit { path: "b.rs".into(), match_count: 1, first_line: Some(1) },
            FileHit { path: "a.rs".into(), match_count: 1, first_line: Some(1) },
        ];
        sort_hits(&mut hits);
        assert_eq!(hits[0].path.to_str().unwrap(), "a.rs");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml ordering_`
Expected: FAIL — `sort_hits` not found.

- [ ] **Step 3: Implement** — add to `rust/src/search.rs`:

```rust
/// Content matches first (by descending count), then name-only; ties by path.
pub fn sort_hits(hits: &mut [FileHit]) {
    hits.sort_by(|a, b| {
        let a_name_only = a.first_line.is_none();
        let b_name_only = b.first_line.is_none();
        a_name_only
            .cmp(&b_name_only)
            .then_with(|| b.match_count.cmp(&a.match_count))
            .then_with(|| a.path.cmp(&b.path))
    });
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml ordering_`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/search.rs
git commit -m "feat: result ordering (content-first, by count, path tiebreak)"
```

---

## Task 7: Tree walk + aggregation with cancellation

**Files:**
- Modify: `rust/src/search.rs`
- Create: `rust/tests/search_tests.rs`

- [ ] **Step 1: Write the failing integration test** — create `rust/tests/search_tests.rs`:

```rust
use srchr::search::{search, Query};
use std::io::Write;
use std::path::Path;

fn write(dir: &Path, name: &str, body: &str) {
    let p = dir.join(name);
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let mut f = std::fs::File::create(&p).unwrap();
    f.write_all(body.as_bytes()).unwrap();
}

fn cancel_never() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false))
}

#[test]
fn merges_name_and_content_hits_deduped() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "alpha.rs", "alpha token\n");   // name AND content
    write(dir.path(), "other.rs", "has alpha inside\n"); // content only
    write(dir.path(), "alpha_only.txt", "nothing\n");    // name only
    let q = Query::compile("alpha").unwrap();
    let hits = search(&q, dir.path(), &cancel_never());

    let by_name: std::collections::HashMap<_, _> = hits
        .iter()
        .map(|h| (h.path.file_name().unwrap().to_str().unwrap().to_string(), h.clone()))
        .collect();

    // alpha.rs appears once, with content match info
    let a = &by_name["alpha.rs"];
    assert_eq!(a.match_count, 1);
    assert_eq!(a.first_line, Some(1));

    // name-only hit present with 0 count / no line
    let n = &by_name["alpha_only.txt"];
    assert_eq!(n.match_count, 0);
    assert_eq!(n.first_line, None);

    // exactly three distinct files
    assert_eq!(hits.len(), 3);
}

#[test]
fn respects_gitignore() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), ".gitignore", "ignored/\n");
    write(dir.path(), "ignored/secret.rs", "alpha\n");
    write(dir.path(), "kept.rs", "alpha\n");
    let q = Query::compile("alpha").unwrap();
    let hits = search(&q, dir.path(), &cancel_never());
    let names: Vec<_> = hits
        .iter()
        .map(|h| h.path.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    assert!(names.contains(&"kept.rs".to_string()));
    assert!(!names.contains(&"secret.rs".to_string()));
}

#[test]
fn cancel_flag_returns_empty() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "a.rs", "alpha\n");
    let q = Query::compile("alpha").unwrap();
    let cancel = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(true));
    let hits = search(&q, dir.path(), &cancel);
    assert!(hits.is_empty());
}
```

- [ ] **Step 2: Expose the crate library** — the integration test uses `srchr::search`, so add a library target. Create `rust/src/lib.rs`:

```rust
pub mod search;
pub mod editor;
pub mod preview;
```

Then add to `rust/Cargo.toml` under `[[bin]]` (append a lib section):

```toml
[lib]
name = "srchr"
path = "src/lib.rs"
```

And change `rust/src/main.rs`'s `mod search;` line to use the library crate:

```rust
use srchr::search;
```

(Create `rust/src/editor.rs` and `rust/src/preview.rs` as empty files for now so `lib.rs` compiles: each may contain just `// filled in later`.)

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml --test search_tests`
Expected: FAIL — `search` function not found.

- [ ] **Step 4: Implement** — add to `rust/src/search.rs`:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use ignore::WalkBuilder;

/// Walk `root` (gitignore-aware), producing one FileHit per matching file.
/// Returns empty if `cancel` is set. Cancellation is checked per entry.
pub fn search(query: &Query, root: &Path, cancel: &Arc<AtomicBool>) -> Vec<FileHit> {
    let mut hits: Vec<FileHit> = Vec::new();

    for result in WalkBuilder::new(root).build() {
        if cancel.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue, // skip unreadable entries silently
        };
        if !entry.file_type().map_or(false, |ft| ft.is_file()) {
            continue;
        }
        let path = entry.path();

        let name_hit = name_matches(query, path);
        let (count, first) = search_file_content(query, path).unwrap_or((0, None));

        if count > 0 || name_hit {
            hits.push(FileHit {
                path: path.to_path_buf(),
                match_count: count,
                first_line: first,
            });
        }
    }

    sort_hits(&mut hits);
    hits
}
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml --test search_tests`
Expected: PASS (all three).

- [ ] **Step 6: Run the full suite and clippy**

Run: `cargo test --manifest-path rust/Cargo.toml && cargo clippy --manifest-path rust/Cargo.toml -- -D warnings`
Expected: all tests PASS, no clippy warnings.

- [ ] **Step 7: Commit**

```bash
git add rust/Cargo.toml rust/src/lib.rs rust/src/main.rs rust/src/search.rs rust/src/editor.rs rust/src/preview.rs rust/tests/search_tests.rs
git commit -m "feat: gitignore-aware walk merging name and content hits"
```

---

## Task 8: Editor resolution, path safety, and arg vector

**Files:**
- Modify: `rust/src/editor.rs`

- [ ] **Step 1: Write the failing tests** — replace the placeholder contents of `rust/src/editor.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_leaves_plain_path() {
        assert_eq!(normalize_path("src/main.rs"), "src/main.rs");
    }

    #[test]
    fn normalize_guards_leading_plus() {
        assert_eq!(normalize_path("+weird.rs"), "./+weird.rs");
    }

    #[test]
    fn normalize_guards_leading_dash() {
        assert_eq!(normalize_path("-weird.rs"), "./-weird.rs");
    }

    #[test]
    fn args_with_line_prepend_plus_line() {
        assert_eq!(editor_args("src/main.rs", Some(42)), vec!["+42", "src/main.rs"]);
    }

    #[test]
    fn args_without_line_just_path() {
        assert_eq!(editor_args("src/main.rs", None), vec!["src/main.rs"]);
    }

    #[test]
    fn args_apply_path_guard() {
        assert_eq!(editor_args("-weird.rs", Some(3)), vec!["+3", "./-weird.rs"]);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path rust/Cargo.toml --lib editor`
Expected: FAIL — functions not found.

- [ ] **Step 3: Implement** — add above the `tests` module in `rust/src/editor.rs`:

```rust
/// Rewrite paths that begin with `+` or `-` to `./...` so editors don't treat
/// them as commands/options.
pub fn normalize_path(path: &str) -> String {
    if path.starts_with('+') || path.starts_with('-') {
        format!("./{path}")
    } else {
        path.to_string()
    }
}

/// Build the argument vector for the editor. Content hits jump to `+line`.
pub fn editor_args(path: &str, line: Option<usize>) -> Vec<String> {
    let safe = normalize_path(path);
    match line {
        Some(n) => vec![format!("+{n}"), safe],
        None => vec![safe],
    }
}

/// Resolve `$EDITOR`; error (rather than guess) if unset or empty.
pub fn resolve_editor() -> Result<String, String> {
    match std::env::var("EDITOR") {
        Ok(e) if !e.trim().is_empty() => Ok(e),
        _ => Err("$EDITOR is not set".to_string()),
    }
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml --lib editor`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/editor.rs
git commit -m "feat: editor resolution, path-safety guard, arg vector"
```

---

## Task 9: Editor launch

**Files:**
- Modify: `rust/src/editor.rs`

- [ ] **Step 1: Write the failing test** — add to the `tests` module in `rust/src/editor.rs` (uses a fake editor that records its args):

```rust
    #[test]
    fn launch_invokes_editor_with_args() {
        use std::io::Read;
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("args.txt");
        // Fake editor: a shell script that writes its args to a file.
        let script = dir.path().join("fakeed.sh");
        std::fs::write(
            &script,
            format!("#!/bin/sh\nprintf '%s\\n' \"$@\" > {}\n", out.display()),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        }

        let status = launch(script.to_str().unwrap(), &editor_args("file.rs", Some(7))).unwrap();
        assert!(status.success());

        let mut s = String::new();
        std::fs::File::open(&out).unwrap().read_to_string(&mut s).unwrap();
        assert_eq!(s, "+7\nfile.rs\n");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml --lib launch_invokes`
Expected: FAIL — `launch` not found.

- [ ] **Step 3: Implement** — add to `rust/src/editor.rs`:

```rust
use std::process::{Command, ExitStatus};

/// Spawn the editor as a foreground child inheriting stdio (it owns the tty),
/// and wait for it to exit. The terminal must already be restored by the caller.
pub fn launch(editor: &str, args: &[String]) -> std::io::Result<ExitStatus> {
    Command::new(editor).args(args).status()
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml --lib launch_invokes`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/editor.rs
git commit -m "feat: launch editor as inheriting foreground child"
```

---

## Task 10: Preview line-range math and data assembly

**Files:**
- Modify: `rust/src/preview.rs`

- [ ] **Step 1: Write the failing tests** — replace the placeholder contents of `rust/src/preview.rs` with:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::Path;

    fn write(dir: &Path, name: &str, body: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(body.as_bytes()).unwrap();
        p
    }

    #[test]
    fn start_is_three_above_the_match() {
        assert_eq!(preview_start(Some(10)), 7);
    }

    #[test]
    fn start_clamps_to_one_near_top() {
        assert_eq!(preview_start(Some(2)), 1);
        assert_eq!(preview_start(Some(1)), 1);
    }

    #[test]
    fn start_is_one_for_name_only() {
        assert_eq!(preview_start(None), 1);
    }

    #[test]
    fn content_hit_starts_above_match_and_highlights() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "a.rs", "l1\nl2\nl3\nl4\nl5\nMATCH\nl7\n");
        let data = build_preview(&p, Some(6), 100).unwrap();
        assert_eq!(data.highlight, Some(6));
        // starts 3 above line 6 => line 3
        assert_eq!(data.lines.first().unwrap().0, 3);
        assert!(data.lines.iter().any(|(n, t)| *n == 6 && t == "MATCH"));
    }

    #[test]
    fn name_only_hit_starts_at_top_no_highlight() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "a.rs", "first\nsecond\n");
        let data = build_preview(&p, None, 100).unwrap();
        assert_eq!(data.highlight, None);
        assert_eq!(data.lines.first().unwrap().0, 1);
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test --manifest-path rust/Cargo.toml --lib preview`
Expected: FAIL — `preview_start` / `build_preview` not found.

- [ ] **Step 3: Implement** — add above the `tests` module in `rust/src/preview.rs`:

```rust
use std::io::{BufRead, BufReader};
use std::path::Path;

/// Mirrors the shell `start=$((line > 3 ? line - 3 : 1))`.
pub fn preview_start(first_line: Option<usize>) -> usize {
    match first_line {
        Some(n) if n > 3 => n - 3,
        _ => 1,
    }
}

/// Lines selected for preview plus which line to highlight.
pub struct PreviewData {
    /// (1-based line number, text) pairs, starting at `preview_start`.
    pub lines: Vec<(usize, String)>,
    pub highlight: Option<usize>,
}

/// Read up to `max_lines` lines from the file starting at `preview_start`.
pub fn build_preview(
    path: &Path,
    first_line: Option<usize>,
    max_lines: usize,
) -> std::io::Result<PreviewData> {
    let start = preview_start(first_line);
    let reader = BufReader::new(std::fs::File::open(path)?);
    let mut lines = Vec::new();
    for (idx, line) in reader.lines().enumerate() {
        let lnum = idx + 1;
        if lnum < start {
            continue;
        }
        if lines.len() >= max_lines {
            break;
        }
        lines.push((lnum, line.unwrap_or_default()));
    }
    Ok(PreviewData { lines, highlight: first_line })
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml --lib preview`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add rust/src/preview.rs
git commit -m "feat: preview line-range math and data assembly"
```

---

## Task 11: Syntect styling into ratatui lines

**Files:**
- Modify: `rust/src/preview.rs`

- [ ] **Step 1: Write the failing test** — add to the `tests` module in `rust/src/preview.rs`:

```rust
    #[test]
    fn styled_lines_match_input_line_count_and_mark_highlight() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "a.rs", "fn main() {}\nlet x = 1;\nMATCH\n");
        let data = build_preview(&p, Some(3), 100).unwrap();
        let styled = style_preview(&data, "a.rs");
        // one styled line per source line
        assert_eq!(styled.lines.len(), data.lines.len());
        // the highlighted row is recorded for the UI
        assert_eq!(styled.highlight_index, Some(2)); // line 3 is the 3rd row (index 2)
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml --lib styled_lines`
Expected: FAIL — `style_preview` / `StyledPreview` not found.

- [ ] **Step 3: Implement** — add to `rust/src/preview.rs`. This converts syntect highlighting into `ratatui::text::Line`s and records which row (index into `lines`) is the highlighted match so the UI can paint its background.

```rust
use once_cell::sync::Lazy;
use ratatui::style::{Color as TuiColor, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAXES: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);
static THEMES: Lazy<ThemeSet> = Lazy::new(ThemeSet::load_defaults);
const DEFAULT_THEME: &str = "base16-ocean.dark";

pub struct StyledPreview {
    pub lines: Vec<Line<'static>>,
    /// Index into `lines` of the match row, if any.
    pub highlight_index: Option<usize>,
}

fn syn_to_tui(color: syntect::highlighting::Color) -> TuiColor {
    TuiColor::Rgb(color.r, color.g, color.b)
}

/// Highlight preview lines with syntect, choosing syntax by file name/extension.
pub fn style_preview(data: &PreviewData, file_name: &str) -> StyledPreview {
    let syntax = SYNTAXES
        .find_syntax_for_file(file_name)
        .ok()
        .flatten()
        .unwrap_or_else(|| SYNTAXES.find_syntax_plain_text());
    let theme = &THEMES.themes[DEFAULT_THEME];
    let mut hl = HighlightLines::new(syntax, theme);

    let mut out_lines: Vec<Line<'static>> = Vec::with_capacity(data.lines.len());
    let mut highlight_index: Option<usize> = None;

    for (row, (lnum, text)) in data.lines.iter().enumerate() {
        if Some(*lnum) == data.highlight {
            highlight_index = Some(row);
        }
        let ranges: Vec<(SynStyle, &str)> = hl
            .highlight_line(text, &SYNTAXES)
            .unwrap_or_else(|_| vec![(SynStyle::default(), text.as_str())]);
        let spans: Vec<Span<'static>> = ranges
            .into_iter()
            .map(|(style, piece)| {
                Span::styled(
                    piece.to_string(),
                    Style::default().fg(syn_to_tui(style.foreground)),
                )
            })
            .collect();
        out_lines.push(Line::from(spans));
    }

    StyledPreview { lines: out_lines, highlight_index }
}
```

- [ ] **Step 4: Add `once_cell` dependency** — in `rust/Cargo.toml` under `[dependencies]`:

```toml
once_cell = "1"
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml --lib styled_lines`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add rust/Cargo.toml rust/src/preview.rs
git commit -m "feat: syntect syntax highlighting into ratatui lines"
```

---

## Task 12: App state machine

**Files:**
- Create: `rust/src/app.rs`
- Modify: `rust/src/lib.rs` (add `pub mod app;`)

- [ ] **Step 1: Write the failing tests** — create `rust/src/app.rs`:

```rust
use crate::search::FileHit;

/// UI state, independent of rendering and I/O.
pub struct App {
    pub query: String,
    pub results: Vec<FileHit>,
    pub selected: usize,
    pub status: String,
}

impl App {
    pub fn new() -> Self {
        App { query: String::new(), results: Vec::new(), selected: 0, status: String::new() }
    }

    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
    }

    pub fn backspace(&mut self) {
        self.query.pop();
    }

    /// Replace results (from a completed search) and clamp the selection.
    pub fn set_results(&mut self, results: Vec<FileHit>) {
        self.results = results;
        if self.selected >= self.results.len() {
            self.selected = self.results.len().saturating_sub(1);
        }
    }

    pub fn move_down(&mut self) {
        if self.selected + 1 < self.results.len() {
            self.selected += 1;
        }
    }

    pub fn move_up(&mut self) {
        self.selected = self.selected.saturating_sub(1);
    }

    pub fn selected_hit(&self) -> Option<&FileHit> {
        self.results.get(self.selected)
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn hit(name: &str) -> FileHit {
        FileHit { path: PathBuf::from(name), match_count: 1, first_line: Some(1) }
    }

    #[test]
    fn typing_and_backspace_edit_query() {
        let mut app = App::new();
        app.push_char('a');
        app.push_char('b');
        app.backspace();
        assert_eq!(app.query, "a");
    }

    #[test]
    fn set_results_clamps_selection() {
        let mut app = App::new();
        app.set_results(vec![hit("a"), hit("b"), hit("c")]);
        app.selected = 2;
        app.set_results(vec![hit("a")]);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn selection_moves_within_bounds() {
        let mut app = App::new();
        app.set_results(vec![hit("a"), hit("b")]);
        app.move_up(); // stays at 0
        assert_eq!(app.selected, 0);
        app.move_down();
        assert_eq!(app.selected, 1);
        app.move_down(); // stays at 1
        assert_eq!(app.selected, 1);
    }
}
```

- [ ] **Step 2: Register the module** — add to `rust/src/lib.rs`:

```rust
pub mod app;
```

- [ ] **Step 3: Run the tests to verify they pass**

Run: `cargo test --manifest-path rust/Cargo.toml --lib app`
Expected: PASS (the implementation is included in Step 1 alongside the tests).

- [ ] **Step 4: Commit**

```bash
git add rust/src/app.rs rust/src/lib.rs
git commit -m "feat: app state machine (query, results, selection)"
```

---

## Task 13: UI rendering

**Files:**
- Create: `rust/src/ui.rs`
- Modify: `rust/src/lib.rs` (add `pub mod ui;`)

- [ ] **Step 1: Implement rendering** — create `rust/src/ui.rs`. Rendering is validated manually (TUI can't be auto-tested); keep it a pure function of `&App` plus a rendered preview.

```rust
use crate::app::App;
use crate::preview::StyledPreview;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

/// Render the whole UI. `preview` is the styled preview of the selected row.
pub fn render(f: &mut Frame, app: &App, preview: &StyledPreview) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    // Query box
    let query = Paragraph::new(format!("> {}", app.query))
        .block(Block::default().borders(Borders::ALL).title("query"));
    f.render_widget(query, chunks[0]);

    // Middle: results | preview
    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|h| {
            let path = h.path.to_string_lossy();
            let tag = if h.first_line.is_none() {
                "[name]".to_string()
            } else {
                format!("({})", h.match_count)
            };
            ListItem::new(Line::from(vec![
                Span::raw(path.into_owned()),
                Span::raw("  "),
                Span::styled(tag, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect();

    let mut state = ListState::default();
    if !app.results.is_empty() {
        state.select(Some(app.selected));
    }
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title("results"))
        .highlight_style(Style::default().add_modifier(Modifier::REVERSED));
    f.render_stateful_widget(list, mid[0], &mut state);

    // Preview, with the match row background-highlighted.
    let preview_lines: Vec<Line> = preview
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if Some(i) == preview.highlight_index {
                let mut l = line.clone();
                l = l.style(Style::default().bg(Color::Rgb(60, 60, 80)));
                l
            } else {
                line.clone()
            }
        })
        .collect();
    let preview_widget =
        Paragraph::new(preview_lines).block(Block::default().borders(Borders::ALL).title("preview"));
    f.render_widget(preview_widget, mid[1]);

    // Status line
    let status = Paragraph::new(app.status.clone()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[2]);
}
```

- [ ] **Step 2: Register the module** — add to `rust/src/lib.rs`:

```rust
pub mod ui;
```

- [ ] **Step 3: Build to verify it compiles**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: compiles (may need minor ratatui API adjustments for the pinned version — fix any signature mismatches, e.g. `f.area()` vs `f.size()`).

- [ ] **Step 4: Commit**

```bash
git add rust/src/ui.rs rust/src/lib.rs
git commit -m "feat: ratatui rendering of query, results, and preview"
```

---

## Task 14: Main event loop — TTY check, debounce, background search, editor handoff

**Files:**
- Modify: `rust/src/main.rs`

- [ ] **Step 1: Implement the event loop** — replace `rust/src/main.rs` with:

```rust
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::execute;
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use srchr::app::App;
use srchr::editor;
use srchr::preview::{build_preview, style_preview, PreviewData, StyledPreview};
use srchr::search::{search, FileHit, Query};

/// A completed search's results, tagged with the query that produced them.
struct SearchResult {
    query: String,
    hits: Vec<FileHit>,
}

const DEBOUNCE: Duration = Duration::from_millis(60);
const PREVIEW_MAX_LINES: usize = 400;

fn main() {
    if !io::stdout().is_terminal() {
        eprintln!("srchr: not a terminal (this is an interactive tool)");
        std::process::exit(2);
    }

    let root = std::env::args().nth(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));

    if let Err(e) = run(root) {
        eprintln!("srchr: {e}");
        std::process::exit(1);
    }
}

fn run(root: PathBuf) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new();
    let (result_tx, result_rx): (Sender<SearchResult>, Receiver<SearchResult>) = mpsc::channel();
    let cancel = Arc::new(AtomicBool::new(false));

    let mut pending_query: Option<String> = None;
    let mut pending_at = Instant::now();
    let mut launch_target: Option<(String, Option<usize>)> = None;

    loop {
        // Drain completed searches; keep only results for the current query.
        while let Ok(res) = result_rx.try_recv() {
            if res.query == app.query {
                app.set_results(res.hits);
                app.status = format!("{} files", app.results.len());
            }
        }

        // Fire a debounced search if the query settled.
        if let Some(q) = pending_query.clone() {
            if pending_at.elapsed() >= DEBOUNCE {
                pending_query = None;
                spawn_search(&q, &root, &cancel, result_tx.clone());
            }
        }

        // Render.
        let styled = current_preview(&app);
        terminal.draw(|f| srchr::ui::render(f, &app, &styled))?;

        // Poll input with a short timeout so the debounce/results loop keeps ticking.
        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                match handle_key(key, &mut app) {
                    Action::Quit => break,
                    Action::Open => {
                        if let Some(hit) = app.selected_hit() {
                            launch_target =
                                Some((hit.path.to_string_lossy().into_owned(), hit.first_line));
                        }
                        break;
                    }
                    Action::QueryChanged => {
                        cancel.store(true, Ordering::Relaxed); // cancel in-flight
                        let fresh = Arc::new(AtomicBool::new(false));
                        // Reset the shared cancel by swapping in a fresh flag.
                        // (See note below: we clone a fresh Arc per search instead.)
                        let _ = fresh;
                        pending_query = Some(app.query.clone());
                        pending_at = Instant::now();
                        if app.query.is_empty() {
                            app.set_results(Vec::new());
                            app.status.clear();
                            pending_query = None;
                        }
                    }
                    Action::None => {}
                }
            }
        }
    }

    // Restore the terminal BEFORE launching the editor.
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Some((path, line)) = launch_target {
        let ed = editor::resolve_editor().map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
        editor::launch(&ed, &editor::editor_args(&path, line))?;
    }

    Ok(())
}

enum Action {
    None,
    Quit,
    Open,
    QueryChanged,
}

fn handle_key(key: KeyEvent, app: &mut App) -> Action {
    match (key.code, key.modifiers) {
        (KeyCode::Esc, _) => Action::Quit,
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Enter, _) => Action::Open,
        (KeyCode::Down, _) | (KeyCode::Char('n'), KeyModifiers::CONTROL) => {
            app.move_down();
            Action::None
        }
        (KeyCode::Up, _) | (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
            app.move_up();
            Action::None
        }
        (KeyCode::Backspace, _) => {
            app.backspace();
            Action::QueryChanged
        }
        (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) => {
            app.push_char(c);
            Action::QueryChanged
        }
        _ => Action::None,
    }
}

/// Build the styled preview for the currently selected row.
fn current_preview(app: &App) -> StyledPreview {
    match app.selected_hit() {
        Some(hit) => {
            let name = hit
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            match build_preview(&hit.path, hit.first_line, PREVIEW_MAX_LINES) {
                Ok(data) => style_preview(&data, &name),
                Err(_) => empty_preview("<unreadable>"),
            }
        }
        None => empty_preview(""),
    }
}

fn empty_preview(msg: &str) -> StyledPreview {
    let data = PreviewData { lines: vec![(1, msg.to_string())], highlight: None };
    style_preview(&data, "")
}

/// Spawn a background search with its own fresh cancel flag.
fn spawn_search(query: &str, root: &PathBuf, cancel: &Arc<AtomicBool>, tx: Sender<SearchResult>) {
    // Reset the shared flag for the new search.
    cancel.store(false, Ordering::Relaxed);
    let cancel = Arc::clone(cancel);
    let query = query.to_string();
    let root = root.clone();
    thread::spawn(move || {
        let hits = match Query::compile(&query) {
            Ok(q) => search(&q, &root, &cancel),
            Err(_) => Vec::new(), // invalid/partial regex: show nothing
        };
        let _ = tx.send(SearchResult { query, hits });
    });
}
```

> **Implementation note for the engineer:** the cancellation model above shares one `AtomicBool`. On `QueryChanged` we set it to `true` to signal any in-flight search to stop, and `spawn_search` resets it to `false` for the new run. Because searches are debounced (only the settled query spawns), at most one search is typically in flight. If you observe races (a stale search resetting the flag), switch to a per-search `Arc<AtomicBool>` stored in a small struct and compare a generation counter before applying results. Keep the `res.query == app.query` guard regardless — it is the correctness backstop that discards stale results.

- [ ] **Step 2: Build**

Run: `cargo build --manifest-path rust/Cargo.toml`
Expected: compiles. Fix any pinned-version API mismatches (e.g. `Frame` generics, `f.area()`).

- [ ] **Step 3: Run the full test suite + clippy + fmt**

Run: `cargo test --manifest-path rust/Cargo.toml && cargo clippy --manifest-path rust/Cargo.toml -- -D warnings && cargo fmt --manifest-path rust/Cargo.toml -- --check`
Expected: tests PASS, no clippy warnings, formatting clean. (If fmt fails, run `cargo fmt --manifest-path rust/Cargo.toml` and re-commit.)

- [ ] **Step 4: Manual smoke test (requires a TTY — ask the user)**

Ask the user to run:
```bash
cargo run --manifest-path rust/Cargo.toml -- .
```
and confirm: typing filters live; results show counts / `[name]`; arrows move selection and update preview; the match line is highlighted; Enter opens `$EDITOR` at the right line; Esc quits cleanly with the terminal restored.

- [ ] **Step 5: Commit**

```bash
git add rust/src/main.rs
git commit -m "feat: live-grep TUI event loop with debounce and editor handoff"
```

---

## Task 15: CI + edge-case hardening

**Files:**
- Modify: `.github/workflows/` (add or extend a workflow with a Rust job)
- Modify: `rust/src/main.rs` and `rust/src/preview.rs` (edge cases below)

- [ ] **Step 1: Add binary-file preview placeholder test** — add to the `tests` module in `rust/src/preview.rs`:

```rust
    #[test]
    fn binary_content_yields_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bin.dat");
        std::fs::write(&p, [0u8, 159, 146, 150, 0, 1, 2]).unwrap();
        let data = build_preview_safe(&p, None, 100);
        assert!(data.lines.iter().any(|(_, t)| t.contains("binary")));
    }
```

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path rust/Cargo.toml --lib binary_content`
Expected: FAIL — `build_preview_safe` not found.

- [ ] **Step 3: Implement `build_preview_safe`** — add to `rust/src/preview.rs`, and switch `main.rs::current_preview` to call it:

```rust
/// Like `build_preview`, but detects binary/unreadable files and returns a
/// placeholder instead of garbage.
pub fn build_preview_safe(path: &Path, first_line: Option<usize>, max_lines: usize) -> PreviewData {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => {
            return PreviewData { lines: vec![(1, "<unreadable file>".into())], highlight: None }
        }
    };
    if bytes.iter().take(8192).any(|b| *b == 0) {
        return PreviewData { lines: vec![(1, "<binary file>".into())], highlight: None };
    }
    build_preview(path, first_line, max_lines).unwrap_or(PreviewData {
        lines: vec![(1, "<unreadable file>".into())],
        highlight: None,
    })
}
```

Then in `rust/src/main.rs`, change `current_preview` to use it:

```rust
            let data = build_preview_safe(&hit.path, hit.first_line, PREVIEW_MAX_LINES);
            style_preview(&data, &name)
```

(and update the `use srchr::preview::...` import to bring in `build_preview_safe` instead of `build_preview`.)

- [ ] **Step 4: Run to verify it passes**

Run: `cargo test --manifest-path rust/Cargo.toml --lib binary_content`
Expected: PASS.

- [ ] **Step 5: Add a Rust CI job** — create `.github/workflows/rust.yml`:

```yaml
name: rust
on:
  push:
  pull_request:
jobs:
  build-test:
    runs-on: ubuntu-latest
    defaults:
      run:
        working-directory: rust
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      - run: cargo fmt -- --check
      - run: cargo clippy -- -D warnings
      - run: cargo test
```

- [ ] **Step 6: Verify locally then commit**

Run: `cargo fmt --manifest-path rust/Cargo.toml -- --check && cargo clippy --manifest-path rust/Cargo.toml -- -D warnings && cargo test --manifest-path rust/Cargo.toml`
Expected: all green.

```bash
git add .github/workflows/rust.yml rust/src/preview.rs rust/src/main.rs
git commit -m "ci: rust job; feat: binary/unreadable preview placeholders"
```

---

## Task 16: Retire the shell ports

> Only after the user confirms the binary reaches parity via the manual smoke test in Task 14.

**Files:**
- Modify: `AGENTS.md`
- Modify: `README.md` (if present)
- Delete: `srchr.fish`, `srchr.sh`, `tests/smoke.sh` (shell-specific)

- [ ] **Step 1: Confirm parity** — ask the user to confirm the manual smoke test passed and they are ready to remove the shell versions. Do not proceed without confirmation.

- [ ] **Step 2: Update `AGENTS.md`** — remove the "keep the ports in sync" invariant and the shell-specific structure/verification sections; document the Rust crate layout, `cargo test`/`clippy`/`fmt` verification, and the manual TUI smoke-test caveat instead. (Write the new content to match the actual final module set.)

- [ ] **Step 3: Remove shell files**

```bash
git rm srchr.fish srchr.sh tests/smoke.sh
```

- [ ] **Step 4: Verify the Rust suite still passes**

Run: `cargo test --manifest-path rust/Cargo.toml`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add AGENTS.md
git commit -m "chore: retire shell ports in favor of the rust binary"
```

---

## Self-Review

**Spec coverage:**
- Self-contained, only `$EDITOR` spawned → Tasks 4–7 (ignore/grep, no fd/rg), 11 (syntect, no bat), 9/14 (editor). ✔
- Live-grep, debounced → Task 14 (`DEBOUNCE`, background thread, channel). ✔
- Rows = files with match counts; `[name]` for name-only → Tasks 6, 12, 13. ✔
- Merged name + content, deduped → Task 7. ✔
- Smart-case regex (content + filename) → Tasks 2, 3, 5. ✔
- Preview: first-match highlight, 3 lines above; name-only from top → Tasks 10, 11. ✔
- Enter opens `+line file` / `file`; path-safety guard → Tasks 8, 9, 14. ✔
- `$EDITOR` unset → error (no guess) → Task 8. ✔
- Edge cases: invalid regex, empty query, no matches, unreadable/binary, no TTY → Tasks 14, 15. ✔
- Testing: cargo test integration + unit, clippy/fmt, CI, manual TUI caveat → Tasks 7–15. ✔
- Fixed default theme (bat-theme deferred) → Task 11 (`DEFAULT_THEME`). ✔
- Retire shell ports after parity → Task 16. ✔

**Placeholder scan:** No "TBD"/"handle edge cases"-style gaps; every code step shows code. The two "adjust for pinned API" notes (Tasks 13, 14) are legitimate version-drift cautions, not missing content.

**Type consistency:** `FileHit { path, match_count, first_line }`, `Query`, `PreviewData { lines, highlight }`, `StyledPreview { lines, highlight_index }`, and `App` methods are used consistently across search/preview/app/ui/main. `build_preview` (Task 10) is superseded by `build_preview_safe` (Task 15) in `main`, with the import change called out explicitly.

**Known follow-ups (out of scope, per spec):** parallel walk optimization, `PgUp`/`PgDn` preview scrolling, `--theme`/`$BAT_THEME`, per-editor line-jump mappings.
