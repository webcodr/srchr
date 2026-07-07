use std::io::{BufRead, BufReader, Read};
use std::path::Path;

use once_cell::sync::Lazy;
use ratatui::style::{Color as TuiColor, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

static SYNTAXES: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);

/// Bundled Tokyo Night ("Night" variant) syntax theme, sourced from
/// `folke/tokyonight.nvim` (Apache-2.0). See `rust/assets/README.md`.
const TOKYO_NIGHT_THEME_BYTES: &[u8] = include_bytes!("../assets/tokyonight_night.tmTheme");

static SYNTAX_THEME: Lazy<Theme> = Lazy::new(|| {
    ThemeSet::load_from_reader(&mut std::io::Cursor::new(TOKYO_NIGHT_THEME_BYTES))
        .expect("bundled tokyonight_night.tmTheme should parse")
});

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

const TAB_WIDTH: usize = 4;

/// Expand tabs to spaces (tracking real column position) and drop any other
/// control characters.
///
/// ratatui's `Buffer::set_stringn` filters out control characters entirely
/// *without* advancing the write cursor for them. Because ratatui only
/// resets the buffer from two frames ago (not the one about to be rendered
/// into), any cell a widget fails to overwrite can retain stale content from
/// an earlier frame — so a literal tab or other control character in
/// previewed text can corrupt the display with fragments of a previous
/// preview. Preview text must therefore never contain raw control
/// characters.
fn sanitize_line(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut col = 0usize;
    for ch in line.chars() {
        if ch == '\t' {
            let spaces = TAB_WIDTH - (col % TAB_WIDTH);
            for _ in 0..spaces {
                out.push(' ');
            }
            col += spaces;
        } else if ch.is_control() {
            // Drop stray control characters (e.g. an errant \r) rather than
            // let them reach the renderer.
        } else {
            out.push(ch);
            col += 1;
        }
    }
    out
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
        lines.push((lnum, sanitize_line(&line.unwrap_or_default())));
    }
    Ok(PreviewData {
        lines,
        highlight: first_line,
    })
}

/// Like `build_preview`, but detects binary/unreadable files and returns a
/// placeholder instead of garbage.
pub fn build_preview_safe(path: &Path, first_line: Option<usize>, max_lines: usize) -> PreviewData {
    let mut file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => {
            return PreviewData {
                lines: vec![(1, "<unreadable file>".into())],
                highlight: None,
            }
        }
    };
    let mut prefix = [0u8; 8192];
    let bytes_read = match file.read(&mut prefix) {
        Ok(n) => n,
        Err(_) => {
            return PreviewData {
                lines: vec![(1, "<unreadable file>".into())],
                highlight: None,
            }
        }
    };
    if prefix[..bytes_read].contains(&0) {
        return PreviewData {
            lines: vec![(1, "<binary file>".into())],
            highlight: None,
        };
    }
    build_preview(path, first_line, max_lines).unwrap_or(PreviewData {
        lines: vec![(1, "<unreadable file>".into())],
        highlight: None,
    })
}

#[derive(Clone)]
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
    let mut hl = HighlightLines::new(syntax, &SYNTAX_THEME);

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

    StyledPreview {
        lines: out_lines,
        highlight_index,
    }
}

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

    #[test]
    fn styled_lines_match_input_line_count_and_mark_highlight() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "a.rs", "fn main() {}\nlet x = 1;\nMATCH\n");
        let data = build_preview(&p, Some(3), 100).unwrap();
        let styled = style_preview(&data, "a.rs");
        assert_eq!(styled.lines.len(), data.lines.len());
        assert_eq!(styled.highlight_index, Some(2));
    }

    #[test]
    fn build_preview_expands_tabs_to_spaces() {
        // Raw tab characters are control characters that ratatui's renderer
        // silently drops (without advancing the cursor), which can corrupt
        // the TUI display by leaving stale cells from earlier frames
        // on-screen. Preview text must never contain a literal tab.
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "a.txt", "\tfoo\na\tb\n");
        let data = build_preview(&p, None, 100).unwrap();
        assert!(
            data.lines.iter().all(|(_, t)| !t.contains('\t')),
            "preview lines must not contain raw tab characters: {:?}",
            data.lines
        );
        assert_eq!(data.lines[0].1, "    foo");
        assert_eq!(data.lines[1].1, "a   b");
    }

    #[test]
    fn build_preview_strips_other_control_characters() {
        let dir = tempfile::tempdir().unwrap();
        let p = write(dir.path(), "a.txt", "a\u{7}b\n");
        let data = build_preview(&p, None, 100).unwrap();
        assert_eq!(data.lines[0].1, "ab");
    }

    #[test]
    fn bundled_tokyo_night_theme_parses() {
        // Force the lazy static to evaluate; panics (via .expect) if the
        // bundled asset is missing or malformed.
        Lazy::force(&SYNTAX_THEME);
    }

    #[test]
    fn binary_content_yields_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bin.dat");
        std::fs::write(&p, [0u8, 159, 146, 150, 0, 1, 2]).unwrap();
        let data = build_preview_safe(&p, None, 100);
        assert!(data.lines.iter().any(|(_, t)| t.contains("binary")));
    }

    #[test]
    fn safe_preview_respects_line_cap_for_text_files() {
        let dir = tempfile::tempdir().unwrap();
        let body = (1..=100)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n");
        let p = write(dir.path(), "large.txt", &body);
        let data = build_preview_safe(&p, None, 5);
        assert_eq!(data.lines.len(), 5);
    }
}
