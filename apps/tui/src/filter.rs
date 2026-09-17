//! Files-view filter popup (`F`): narrow the list by name, tags, or lack of tags.
//!
//! Matching lives in [`storey_shared::filter`] so the GUI reuses it;
//! this module keeps the TUI popup chrome, [`App`] key handling, and
//! cursor clamping. Filtering is live — every keystroke re-narrows —
//! so `Enter`/`Esc` only close the popup. `x`/`X` in the files view
//! clears the filter.

use ratatui::{
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};
pub use storey_shared::filter::{FilterKind, FilterState, filter_summary, visible_indices};

use crate::{app::App, dialog};

impl App {
    /// Visible-row indices under the current filter.
    pub fn visible_indices(&self) -> Vec<usize> {
        visible_indices(&self.files.records, &self.files.filter)
    }

    /// Open the filter popup, keeping the previous query and kind.
    pub fn open_filter(&mut self) {
        self.files.filter.open = true;
    }

    /// Close the popup; the narrowing stays applied.
    pub fn close_filter(&mut self) {
        self.files.filter.open = false;
    }

    /// Drop the query, reset to filename mode, and close.
    pub fn clear_filter(&mut self) {
        self.files.filter.clear();
        self.files.sel = 0;
    }

    /// Cycle filename → tags → untagged (Tab) or backwards (Shift-Tab).
    fn cycle_filter(&mut self, delta: i32) {
        self.files.filter.cycle(delta);
        self.clamp_files_sel();
    }

    /// Clamp the cursor to the visible rows.
    fn clamp_files_sel(&mut self) {
        let n = self.visible_indices().len();
        self.files.sel = self.files.sel.min(n.saturating_sub(1));
    }

    /// Keys while the filter popup is open; true quits (never).
    pub fn on_key_filter(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        match code {
            KeyCode::Tab => self.cycle_filter(1),
            KeyCode::BackTab => self.cycle_filter(-1),
            KeyCode::Enter | KeyCode::Esc => self.close_filter(),
            KeyCode::Backspace if self.files.filter.kind != FilterKind::Untagged => {
                self.files.filter.query.pop();
            }
            KeyCode::Char(ch)
                if (mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT)
                    && self.files.filter.kind != FilterKind::Untagged =>
            {
                self.files.filter.query.push(ch);
            }
            _ => {}
        }
        self.clamp_files_sel();
        false
    }
}

/// Centered popup: the three modes plus the query line.
pub fn render_popup(f: &mut ratatui::Frame<'_>, app: &App) {
    let area = f.area();
    let width = 52.min(area.width.saturating_sub(4)).max(30);
    let rect = dialog::centered(area, width, 8);
    f.render_widget(Clear, rect);
    let mut lines = vec![Line::raw("tab switches mode · matches live")];
    for (kind, hint) in [
        (FilterKind::Filename, "match file names"),
        (FilterKind::Tags, "match tag values (cat=val)"),
        (FilterKind::Untagged, "only untagged"),
    ] {
        let marker = if app.files.filter.kind == kind {
            "> "
        } else {
            "  "
        };
        lines.push(Line::raw(format!("{marker}{hint}")));
    }
    lines.push(query_line(app));
    lines.push(Line::raw("enter/esc close"));
    let widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title(title(app)));
    f.render_widget(widget, rect);
}

/// Popup title naming the active kind.
fn title(app: &App) -> String {
    let kind = match app.files.filter.kind {
        FilterKind::Filename => "filename",
        FilterKind::Tags => "tags",
        FilterKind::Untagged => "untagged",
    };
    format!("filter: {kind}")
}

/// Query row, or a placeholder when the kind takes no input.
fn query_line(app: &App) -> Line<'static> {
    if app.files.filter.kind == FilterKind::Untagged {
        Line::raw("query: (untagged takes no input)")
    } else {
        Line::raw(format!("query: {}", app.files.filter.query))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use storey_shared::model::Record;

    fn rec(id: &str, name: &str, tags: &[(&str, &[&str])]) -> Record {
        let tags = tags
            .iter()
            .map(|(c, vs)| (c.to_string(), vs.iter().map(|v| v.to_string()).collect()))
            .collect();
        Record {
            id: id.into(),
            display_name: name.into(),
            original_rel: name.into(),
            stored_rel: String::new(),
            size: 1,
            tags,
        }
    }

    fn files_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-filter-{}", std::process::id()));
        let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
        app.close_error();
        app.is_repo = true;
        app.mode = crate::app::Mode::Files;
        app.files.records = vec![
            rec("a", "Concert.mkv", &[("genre", &["rock"])]),
            rec("b", "interview.mp3", &[("genre", &["Jazz"])]),
            rec("c", "notes.txt", &[]),
        ];
        app
    }

    #[test]
    fn each_kind_matches() {
        let mut app = files_app();
        app.files.filter.query = "concert".into();
        assert_eq!(app.visible_indices(), vec![0]);
        app.files.filter.kind = FilterKind::Tags;
        app.files.filter.query = "jazz".into();
        assert_eq!(app.visible_indices(), vec![1]);
        app.files.filter.query = "genre=roc".into();
        assert_eq!(app.visible_indices(), vec![0]);
        app.files.filter.kind = FilterKind::Untagged;
        assert_eq!(app.visible_indices(), vec![2]);
        app.files.filter.kind = FilterKind::Filename;
        app.files.filter.query.clear();
        assert_eq!(app.visible_indices().len(), 3);
        assert_eq!(filter_summary(&app.files.filter), None);
    }

    #[test]
    fn keys_type_cycle_close_clear() {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mut app = files_app();
        app.open_filter();
        app.on_key_filter(KeyCode::Char('c'), KeyModifiers::NONE);
        assert_eq!(app.visible_indices(), vec![0]);
        app.on_key_filter(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.files.filter.kind, FilterKind::Tags);
        app.on_key_filter(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(app.files.filter.kind, FilterKind::Untagged);
        assert_eq!(app.visible_indices(), vec![2]);
        app.on_key_filter(KeyCode::Enter, KeyModifiers::NONE);
        assert!(!app.files.filter.open);
        app.clear_filter();
        assert_eq!(app.visible_indices().len(), 3);
    }

    #[test]
    fn navigation_follows_visible_rows() {
        let mut app = files_app();
        app.files.filter.kind = FilterKind::Untagged;
        app.files.sel = 5;
        app.move_files(1);
        assert_eq!(app.files.sel, 0);
        assert_eq!(app.selected_record().unwrap().id, "c");
    }
}
