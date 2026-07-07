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
    write(dir.path(), "alpha.rs", "alpha token\n");
    write(dir.path(), "other.rs", "has alpha inside\n");
    write(dir.path(), "alpha_only.txt", "nothing\n");
    let q = Query::compile("alpha").unwrap();
    let hits = search(&q, dir.path(), &cancel_never());

    let by_name: std::collections::HashMap<_, _> = hits
        .iter()
        .map(|h| {
            (
                h.path.file_name().unwrap().to_str().unwrap().to_string(),
                h.clone(),
            )
        })
        .collect();

    let a = &by_name["alpha.rs"];
    assert_eq!(a.match_count, 1);
    assert_eq!(a.first_line, Some(1));

    let n = &by_name["alpha_only.txt"];
    assert_eq!(n.match_count, 0);
    assert_eq!(n.first_line, None);

    assert_eq!(hits.len(), 3);
}

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
