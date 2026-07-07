use std::path::PathBuf;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;
use ignore::WalkBuilder;
use regex::RegexBuilder;

/// Smart-case: case-sensitive only when the query contains an uppercase char.
pub fn is_case_sensitive(query: &str) -> bool {
    query.chars().any(|c| c.is_uppercase())
}

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

/// True if the file's basename matches the query (mirrors `fd` default).
pub fn name_matches(query: &Query, path: &Path) -> bool {
    match path.file_name().and_then(|n| n.to_str()) {
        Some(name) => query.name.is_match(name),
        None => false,
    }
}

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

/// Walk `root` (gitignore-aware), producing one FileHit per matching file.
/// Returns empty if `cancel` is set. Cancellation is checked per entry.
pub fn search(query: &Query, root: &Path, cancel: &Arc<AtomicBool>) -> Vec<FileHit> {
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
        assert!(Query::compile("Foo").unwrap().case_sensitive);
    }

    fn write_file(dir: &std::path::Path, name: &str, body: &str) -> PathBuf {
        use std::io::Write;
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

    #[test]
    fn filename_match_uses_basename() {
        let q = Query::compile("config").unwrap();
        assert!(name_matches(&q, Path::new("src/config.rs")));
        assert!(!name_matches(&q, Path::new("src/main.rs")));
    }

    #[test]
    fn filename_match_is_smart_case() {
        let q = Query::compile("readme").unwrap();
        assert!(name_matches(&q, Path::new("README.md")));
        let q2 = Query::compile("README").unwrap();
        assert!(!name_matches(&q2, Path::new("readme.md")));
    }

    #[test]
    fn ordering_content_before_name_only_then_by_count() {
        let mut hits = vec![
            FileHit { path: "z_name.rs".into(), match_count: 0, first_line: None },
            FileHit { path: "b.rs".into(), match_count: 2, first_line: Some(1) },
            FileHit { path: "a.rs".into(), match_count: 5, first_line: Some(3) },
        ];
        sort_hits(&mut hits);
        let order: Vec<_> = hits.iter().map(|h| h.path.to_str().unwrap()).collect();
        assert_eq!(order, vec!["a.rs", "b.rs", "z_name.rs"]);
    }

    #[test]
    fn ordering_breaks_count_ties_by_path() {
        let mut hits = vec![
            FileHit { path: "b.rs".into(), match_count: 1, first_line: Some(1) },
            FileHit { path: "a.rs".into(), match_count: 1, first_line: Some(1) },
        ];
        sort_hits(&mut hits);
        let order: Vec<_> = hits.iter().map(|h| h.path.to_str().unwrap()).collect();
        assert_eq!(order, vec!["a.rs", "b.rs"]);
    }
}
