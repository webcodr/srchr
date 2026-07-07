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
        App {
            query: String::new(),
            results: Vec::new(),
            selected: 0,
            status: String::new(),
        }
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
        FileHit {
            path: PathBuf::from(name),
            match_count: 1,
            first_line: Some(1),
        }
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
        app.move_up();
        assert_eq!(app.selected, 0);
        app.move_down();
        assert_eq!(app.selected, 1);
        app.move_down();
        assert_eq!(app.selected, 1);
    }
}
