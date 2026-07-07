use std::path::PathBuf;
use std::path::Path;
use grep_regex::RegexMatcher;
use grep_searcher::Searcher;
use grep_searcher::sinks::UTF8;
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
}
