//! Files-view destructive ops: permanent remove (`d`) and rename (`m`).
//!
//! Remove asks `y`/`n` first — it deletes view links, the file, and the
//! index record. Rename opens a text buffer prefilled with the current
//! display name; both target the record id.

use ratatui::{
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph},
};

use crate::{app::App, dialog};

impl App {
    /// Drop the Detail buffer's whole category from the record id.
    pub fn remove_category(&mut self) {
        self.unack_error();
        let cat = self.tagging.cat.trim().to_string();
        if cat.is_empty() {
            self.notice = "enter a category (c)".to_string();
            return;
        }
        let id = self.tagging.record_id.clone();
        match self.client.remove_category(&self.cwd, &id, &cat, |_| {}) {
            Ok(_) => {
                self.tagging.cat.clear();
                self.load_records();
                self.notice = "category removed (view stale)".to_string();
            }
            Err(e) => self.fail("tag", &e),
        }
    }

    /// Open the remove-confirm popup for the visible cursor, if any.
    pub fn open_rm(&mut self) {
        if self.selected_record().is_none() {
            return;
        }
        self.files.rm_open = true;
    }

    /// Close the remove popup without deleting.
    pub fn close_rm(&mut self) {
        self.files.rm_open = false;
    }

    /// Delete the selected record's links, file, and index entry.
    pub fn do_rm(&mut self) {
        self.unack_error();
        let Some(record) = self.selected_record().cloned() else {
            self.close_rm();
            return;
        };
        self.close_rm();
        match self.client.remove_file(&self.cwd, &record.id, |_| {}) {
            Ok(_) => {
                self.notice = format!("removed {}", record.display_name);
                self.load_records();
            }
            Err(e) => self.fail("rm", &e),
        }
    }

    /// Open the rename popup, prefilled with the current display name.
    pub fn open_rename(&mut self) {
        let Some(record) = self.selected_record() else {
            return;
        };
        self.files.rename = Some(record.display_name.clone());
    }

    /// Close the rename popup, discarding the buffer.
    pub fn close_rename(&mut self) {
        self.files.rename = None;
    }

    /// Apply the rename buffer via `mv` on the record id.
    pub fn do_rename(&mut self) {
        self.unack_error();
        let Some(id) = self.selected_record().map(|r| r.id.clone()) else {
            self.close_rename();
            return;
        };
        let name = self.files.rename.clone().unwrap_or_default();
        let name = name.trim().to_string();
        if name.is_empty() {
            self.notice = "enter a new name".to_string();
            return;
        }
        self.close_rename();
        match self.client.rename(&self.cwd, &id, &name, |_| {}) {
            Ok(report) => {
                self.notice = format!("renamed {} -> {}", report.old, report.new);
                self.load_records();
            }
            Err(e) => self.fail("mv", &e),
        }
    }

    /// Keys for the remove-confirm popup; true quits.
    pub fn on_key_rm(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        if code == KeyCode::Char('q') && mods == KeyModifiers::NONE {
            return true;
        }
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.do_rm(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => self.close_rm(),
            _ => {}
        }
        false
    }

    /// Keys for the rename buffer; true quits (never).
    pub fn on_key_rename(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        match code {
            KeyCode::Esc => self.close_rename(),
            KeyCode::Enter => self.do_rename(),
            KeyCode::Backspace => rename_pop(self),
            KeyCode::Char(ch) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
                rename_push(self, ch)
            }
            _ => {}
        }
        false
    }
}

/// Backspace the rename buffer, if open.
fn rename_pop(app: &mut App) {
    if let Some(buf) = app.files.rename.as_mut() {
        buf.pop();
    }
}

/// Append a char to the rename buffer, if open.
fn rename_push(app: &mut App, ch: char) {
    if let Some(buf) = app.files.rename.as_mut() {
        buf.push(ch);
    }
}

/// Remove-confirm popup over the files list.
pub fn render_rm_popup(f: &mut ratatui::Frame<'_>, app: &App) {
    let name = app
        .selected_record()
        .map(|r| r.display_name.as_str())
        .unwrap_or("?");
    let lines = vec![
        Line::raw(format!("permanently remove {name} from the system?")),
        Line::raw("y confirms, n cancels"),
    ];
    let area = f.area();
    let rect = dialog::centered(area, 56.min(area.width.saturating_sub(4)).max(30), 6);
    f.render_widget(Clear, rect);
    let widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("remove"));
    f.render_widget(widget, rect);
}

/// Rename popup showing the live buffer.
pub fn render_rename_popup(f: &mut ratatui::Frame<'_>, app: &App) {
    let buf = app.files.rename.clone().unwrap_or_default();
    let lines = vec![
        Line::raw(format!("new name: {buf}")),
        Line::raw("enter applies, esc cancels"),
    ];
    let area = f.area();
    let rect = dialog::centered(area, 56.min(area.width.saturating_sub(4)).max(30), 6);
    f.render_widget(Clear, rect);
    let widget =
        Paragraph::new(lines).block(Block::default().borders(Borders::ALL).title("rename"));
    f.render_widget(widget, rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn files_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-manage-{}", std::process::id()));
        let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
        app.close_error();
        app.is_repo = true;
        app.mode = crate::app::Mode::Files;
        app.files.records = vec![storey_shared::model::Record {
            id: "ab".into(),
            display_name: "a.mkv".into(),
            original_rel: "a.mkv".into(),
            stored_rel: String::new(),
            size: 1,
            tags: BTreeMap::new(),
        }];
        app
    }

    #[test]
    fn remove_category_needs_a_name() {
        let mut app = files_app();
        app.mode = crate::app::Mode::Detail;
        app.tagging.open("ab".into());
        app.remove_category();
        assert_eq!(app.notice, "enter a category (c)");
    }

    #[test]
    fn rm_popup_confirms_or_cancels() {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mut app = files_app();
        app.open_rm();
        assert!(app.files.rm_open);
        app.on_key_rm(KeyCode::Char('n'), KeyModifiers::NONE);
        assert!(!app.files.rm_open);
        app.open_rm();
        app.on_key_rm(KeyCode::Char('y'), KeyModifiers::NONE);
        assert!(!app.files.rm_open);
        assert!(app.notice.contains("no storey binary"));
    }

    #[test]
    fn rename_edits_and_validates() {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mut app = files_app();
        app.open_rename();
        assert_eq!(app.files.rename.as_deref(), Some("a.mkv"));
        app.on_key_rename(KeyCode::Char('2'), KeyModifiers::NONE);
        app.on_key_rename(KeyCode::Backspace, KeyModifiers::NONE);
        app.on_key_rename(KeyCode::Backspace, KeyModifiers::NONE);
        assert_eq!(app.files.rename.as_deref(), Some("a.mk"));
        app.files.rename = Some("  ".into());
        app.on_key_rename(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(app.notice, "enter a new name");
        app.on_key_rename(KeyCode::Esc, KeyModifiers::NONE);
        assert!(app.files.rename.is_none());
    }

    #[test]
    fn popups_need_a_selection() {
        let mut app = files_app();
        app.files.records.clear();
        app.open_rm();
        app.open_rename();
        assert!(!app.files.rm_open);
        assert!(app.files.rename.is_none());
    }
}
