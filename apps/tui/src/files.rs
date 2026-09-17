//! View 1 — repo files: `storey list` rows with update support.
//!
//! [`FilesState`] holds the loaded records plus the cursor. Loading
//! and `update` live here as [`App`] methods; rendering too, so
//! `ui.rs` only dispatches.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};
use storey_shared::model::Record;

use crate::{app::App, filter::FilterState};

/// Loaded `storey list` rows plus the file cursor.
#[derive(Debug, Default)]
pub struct FilesState {
    pub records: Vec<Record>,
    pub sel: usize,
    /// Active narrowing of the list; the cursor tracks visible rows.
    pub filter: FilterState,
    /// Remove-confirm popup visibility (`d`); `y` confirms.
    pub rm_open: bool,
    /// Rename popup buffer (`m`); `None` means closed.
    pub rename: Option<String>,
}

impl FilesState {
    fn clamp(&mut self) {
        let n = crate::filter::visible_indices(&self.records, &self.filter).len();
        self.sel = self.sel.min(n.saturating_sub(1));
    }
}

impl App {
    /// Cycle Browse ↔ Files (detail/plugins step back one level).
    pub(crate) fn toggle_mode(&mut self) {
        match self.mode {
            crate::app::Mode::Browse => self.enter_files(),
            crate::app::Mode::Files => self.back_to_browse(),
            crate::app::Mode::Detail => self.mode = crate::app::Mode::Files,
            crate::app::Mode::Plugins => self.mode = crate::app::Mode::Detail,
        }
    }

    /// Enter the files view, loading records first (repo only).
    pub fn enter_files(&mut self) {
        if !self.is_repo {
            return;
        }
        self.load_records();
        self.mode = crate::app::Mode::Files;
    }

    /// Back to the Browse breadcrumbs.
    pub fn back_to_browse(&mut self) {
        self.mode = crate::app::Mode::Browse;
    }

    /// Reload `storey list` rows; exit 2 just clears (not a library).
    pub fn load_records(&mut self) {
        match self.client.list(&self.cwd, |_| {}) {
            Ok(records) => {
                self.files.records = records;
                self.files.clamp();
            }
            Err(e) => {
                if e.code != Some(2) {
                    self.fail("list", &e);
                }
                self.files.records.clear();
                self.files.sel = 0;
            }
        }
    }

    /// Move the files cursor over visible rows, clamped.
    pub fn move_files(&mut self, delta: i32) {
        let len = self.visible_indices().len() as i32;
        if len == 0 {
            self.files.sel = 0;
            return;
        }
        let next = (self.files.sel as i32 + delta).clamp(0, len - 1);
        self.files.sel = next as usize;
    }

    /// Selected record in the files view, mapped through the filter.
    pub fn selected_record(&self) -> Option<&Record> {
        let visible = self.visible_indices();
        visible
            .get(self.files.sel)
            .and_then(|&i| self.files.records.get(i))
    }

    /// Open the files cursor in the Detail view.
    pub fn open_detail(&mut self) {
        let Some(record) = self.selected_record().cloned() else {
            return;
        };
        self.tagging.open(record.id);
        self.mode = crate::app::Mode::Detail;
    }

    /// Key handling for the files view; true quits.
    pub fn on_key_files(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        if code == KeyCode::Char('q') && mods == KeyModifiers::NONE {
            return true;
        }
        if self.files.filter.open {
            return self.on_key_filter(code, mods);
        }
        if self.files.rm_open {
            return self.on_key_rm(code, mods);
        }
        if self.files.rename.is_some() {
            return self.on_key_rename(code, mods);
        }
        match code {
            KeyCode::Down | KeyCode::Char('j') => self.move_files(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_files(-1),
            KeyCode::Enter => self.open_detail(),
            KeyCode::Char('u') => self.spawn_update(false),
            KeyCode::Char('U') => self.spawn_update(true),
            KeyCode::Char('f') | KeyCode::Char('F') => self.open_filter(),
            KeyCode::Char('x') | KeyCode::Char('X') => self.clear_filter(),
            KeyCode::Char('d') => self.open_rm(),
            KeyCode::Char('m') => self.open_rename(),
            KeyCode::Esc | KeyCode::Char('t') => self.back_to_browse(),
            _ => {}
        }
        false
    }

    /// Click a visible row: select it, then open it for tagging.
    pub fn click_file(&mut self, row: usize) {
        if row < self.visible_indices().len() {
            self.files.sel = row;
            self.open_detail();
        }
    }
}

/// File list widget over visible rows, selected row cyan with `>`.
pub fn render_files(app: &App) -> List<'_> {
    let visible = app.visible_indices();
    let items: Vec<ListItem<'_>> = visible
        .iter()
        .enumerate()
        .filter_map(|(pos, &i)| {
            app.files
                .records
                .get(i)
                .map(|r| ListItem::new(file_line(r, pos == app.files.sel)))
        })
        .collect();
    let title = files_title(app, visible.len());
    if items.is_empty() {
        List::new(vec![ListItem::new(empty_hint(app))])
            .block(Block::default().borders(Borders::ALL).title(title))
    } else {
        List::new(items).block(Block::default().borders(Borders::ALL).title(title))
    }
}

/// Empty-list hint: update when the library is empty, filter help when
/// records exist but the filter matches nothing.
fn empty_hint(app: &App) -> &'static str {
    if app.files.records.is_empty() {
        "no files — press u to update"
    } else {
        "no matches — F edits filter, X clears"
    }
}

/// `files (12)` plain, or `files (3/64) [tags:rock]` when narrowed.
fn files_title(app: &App, shown: usize) -> String {
    let total = app.files.records.len();
    match crate::filter::filter_summary(&app.files.filter) {
        Some(summary) => format!("files ({shown}/{total}) [{summary}]"),
        None => format!("files ({total})"),
    }
}

/// One file row: `name — tag summary`, highlighted when selected.
fn file_line(r: &Record, selected: bool) -> Line<'static> {
    let text = format!("{} — {}", r.display_name, r.display_tags());
    if selected {
        Line::from(vec![
            Span::styled("> ", Style::default()),
            Span::styled(text, Style::default().fg(Color::Cyan)),
        ])
    } else {
        Line::from(vec![Span::raw("  "), Span::raw(text)])
    }
}

#[cfg(test)]
mod tests {
    use crate::app::{App, Mode};

    fn plain_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-tui-files-{}", std::process::id()));
        let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
        app.close_error();
        app
    }

    #[test]
    fn files_move_clamps() {
        let mut app = plain_app();
        app.move_files(1);
        assert_eq!(app.files.sel, 0);
    }

    #[test]
    fn filter_keys_open_and_clear_both_cases() {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mut app = plain_app();
        app.is_repo = true;
        app.mode = Mode::Files;
        app.on_key_files(KeyCode::Char('f'), KeyModifiers::NONE);
        assert!(app.files.filter.open);
        app.close_filter();
        app.on_key_files(KeyCode::Char('F'), KeyModifiers::SHIFT);
        assert!(app.files.filter.open);
        app.close_filter();
        app.files.filter.query = "zzz".into();
        app.on_key_files(KeyCode::Char('x'), KeyModifiers::NONE);
        assert!(app.files.filter.query.is_empty());
        app.files.filter.query = "zzz".into();
        app.on_key_files(KeyCode::Char('X'), KeyModifiers::SHIFT);
        assert!(app.files.filter.query.is_empty());
    }

    #[test]
    fn gated_background_update_reports_summary() {
        let bin = std::env::var("STOREY_BIN").unwrap_or_default();
        if !std::path::Path::new(&bin).is_file() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("storey-tui-files-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        assert!(
            std::process::Command::new(&bin)
                .arg("init")
                .current_dir(&dir)
                .output()
                .unwrap()
                .status
                .success()
        );
        let mut app = App::new(std::path::PathBuf::from(bin), dir.clone());
        app.enter_files();
        assert_eq!(app.mode, Mode::Files);
        assert_eq!(app.files.records.len(), 1);
        app.spawn_update(false);
        for _ in 0..200 {
            app.poll_pending();
            if app.pending.is_none() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(25));
        }
        assert!(app.pending.is_none(), "worker did not finish");
        assert!(app.notice.contains("added"), "notice was {:?}", app.notice);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
