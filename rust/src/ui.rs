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
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(f.area());

    let mid = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[0]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(1)])
        .split(mid[0]);

    let query = Paragraph::new(app.query.to_string())
        .block(Block::default().borders(Borders::ALL).title("files"));
    f.render_widget(query, left[0]);

    let items: Vec<ListItem> = app
        .results
        .iter()
        .map(|h| {
            let path = h.path.to_string_lossy();
            let tag = if h.first_line.is_some() {
                format!("({})", h.match_count)
            } else {
                "".to_string()
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
    f.render_stateful_widget(list, left[1], &mut state);

    let preview_lines: Vec<Line> = preview
        .lines
        .iter()
        .enumerate()
        .map(|(i, line)| {
            if Some(i) == preview.highlight_index {
                line.clone()
                    .style(Style::default().bg(Color::Rgb(60, 60, 80)))
            } else {
                line.clone()
            }
        })
        .collect();
    let preview_widget = Paragraph::new(preview_lines).block(
        Block::default()
            .borders(Borders::ALL)
            .title(preview_title(app)),
    );
    f.render_widget(preview_widget, mid[1]);

    let status = Paragraph::new(app.status.clone()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[1]);
}

/// Title for the preview pane: the selected file's path, or a fallback
/// label when nothing is selected.
fn preview_title(app: &App) -> String {
    match app.selected_hit() {
        Some(hit) => hit.path.to_string_lossy().into_owned(),
        None => "preview".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::FileHit;
    use std::path::PathBuf;

    fn hit(name: &str) -> FileHit {
        FileHit {
            path: PathBuf::from(name),
            match_count: 1,
            first_line: Some(1),
        }
    }

    #[test]
    fn preview_title_shows_selected_file_path() {
        let mut app = App::new();
        app.set_results(vec![hit("src/main.rs"), hit("src/lib.rs")]);
        app.selected = 1;
        assert_eq!(preview_title(&app), "src/lib.rs");
    }

    #[test]
    fn preview_title_falls_back_when_no_results() {
        let app = App::new();
        assert_eq!(preview_title(&app), "preview");
    }
}
