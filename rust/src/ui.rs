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

    let query = Paragraph::new(format!("> {}", app.query))
        .block(Block::default().borders(Borders::ALL).title("query"));
    f.render_widget(query, left[0]);

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
    let preview_widget = Paragraph::new(preview_lines)
        .block(Block::default().borders(Borders::ALL).title("preview"));
    f.render_widget(preview_widget, mid[1]);

    let status = Paragraph::new(app.status.clone()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[1]);
}
