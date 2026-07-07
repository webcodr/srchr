use std::io::{self, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use clap::Parser;

use srchr::app::App;
use srchr::editor;
use srchr::preview::{build_preview_safe, style_preview, PreviewData, StyledPreview};
use srchr::search::{list_dir, search, FileHit, Query};

const DEBOUNCE: Duration = Duration::from_millis(60);
const POLL_INTERVAL: Duration = Duration::from_millis(30);
const PREVIEW_MAX_LINES: usize = 400;

struct SearchResult {
    query: String,
    hits: Vec<FileHit>,
    error: Option<String>,
}

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

fn main() {
    let cli = Cli::parse();

    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        eprintln!("srchr: not a terminal (this is an interactive tool)");
        std::process::exit(2);
    }

    if let Err(e) = run(cli.path, cli.query) {
        eprintln!("srchr: {e}");
        std::process::exit(1);
    }
}

fn run(root: PathBuf, initial_query: Option<String>) -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

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
    let mut current_cancel: Option<Arc<AtomicBool>> = None;
    let mut launch_target: Option<(String, Option<usize>)> = None;
    let mut preview_key: Option<(PathBuf, Option<usize>)> = None;
    let mut styled_preview = empty_preview("");

    let loop_result = (|| -> io::Result<()> {
        loop {
            while let Ok(res) = result_rx.try_recv() {
                if res.query == app.query {
                    app.set_results(res.hits);
                    app.status = match res.error {
                        Some(e) => format!("invalid pattern: {e}"),
                        None => format!("{} files", app.results.len()),
                    };
                }
            }

            if let Some(q) = pending_query.clone() {
                if pending_at.elapsed() >= DEBOUNCE {
                    pending_query = None;
                    if let Some(cancel) = current_cancel.take() {
                        cancel.store(true, Ordering::Relaxed);
                    }
                    current_cancel = Some(spawn_search(&q, &root, result_tx.clone()));
                }
            }

            let selected_key = app
                .selected_hit()
                .map(|hit| (hit.path.clone(), hit.first_line));
            if selected_key != preview_key {
                styled_preview = current_preview(&app);
                preview_key = selected_key;
            }
            terminal.draw(|f| srchr::ui::render(f, &app, &styled_preview))?;

            if event::poll(POLL_INTERVAL)? {
                if let Event::Key(key) = event::read()? {
                    match handle_key(key, &mut app) {
                        Action::Quit => return Ok(()),
                        Action::Open => {
                            if let Some(hit) = app.selected_hit() {
                                launch_target =
                                    Some((hit.path.to_string_lossy().into_owned(), hit.first_line));
                            }
                            return Ok(());
                        }
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
                        Action::None => {}
                    }
                }
            }
        }
    })();

    if let Some(cancel) = current_cancel {
        cancel.store(true, Ordering::Relaxed);
    }

    let raw_result = disable_raw_mode();
    let screen_result = execute!(terminal.backend_mut(), LeaveAlternateScreen);
    let cursor_result = terminal.show_cursor();

    loop_result?;
    raw_result?;
    screen_result?;
    cursor_result?;

    if let Some((path, line)) = launch_target {
        let ed = editor::resolve_editor().map_err(io::Error::other)?;
        let status = editor::launch(&ed, &editor::editor_args(&path, line))?;
        if !status.success() {
            return Err(io::Error::other(format!("editor exited with {status}")));
        }
    }

    Ok(())
}

#[derive(Debug, PartialEq, Eq)]
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
        (KeyCode::Char(c), modifiers) if !modifiers.contains(KeyModifiers::CONTROL) => {
            app.push_char(c);
            Action::QueryChanged
        }
        _ => Action::None,
    }
}

fn current_preview(app: &App) -> StyledPreview {
    match app.selected_hit() {
        Some(hit) => {
            let name = hit
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let data = build_preview_safe(&hit.path, hit.first_line, PREVIEW_MAX_LINES);
            style_preview(&data, &name)
        }
        None => empty_preview(""),
    }
}

fn empty_preview(msg: &str) -> StyledPreview {
    let data = PreviewData {
        lines: vec![(1, msg.to_string())],
        highlight: None,
    };
    style_preview(&data, "")
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_key_edits_query() {
        let mut app = App::new();
        let action = handle_key(
            KeyEvent::new(KeyCode::Char('a'), KeyModifiers::NONE),
            &mut app,
        );
        assert_eq!(action, Action::QueryChanged);
        assert_eq!(app.query, "a");

        let action = handle_key(
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
            &mut app,
        );
        assert_eq!(action, Action::QueryChanged);
        assert_eq!(app.query, "");
    }

    #[test]
    fn handle_key_moves_selection() {
        let mut app = App::new();
        app.set_results(vec![
            FileHit {
                path: "a.rs".into(),
                match_count: 1,
                first_line: Some(1),
            },
            FileHit {
                path: "b.rs".into(),
                match_count: 1,
                first_line: Some(1),
            },
        ]);

        let action = handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE), &mut app);
        assert_eq!(action, Action::None);
        assert_eq!(app.selected, 1);

        let action = handle_key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), &mut app);
        assert_eq!(action, Action::None);
        assert_eq!(app.selected, 0);
    }

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
}
