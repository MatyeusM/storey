//! Browse-first TUI: address bar, repo-aware top bar, dir list.
//!
//! In a library (`is_repo`) subfolders are not browsable: the top bar
//! shows only the folder basename plus `[tag -] >> ... >> [Add +]`
//! breadcrumbs, and `j/k` moves between those chips. Plain
//! directories keep the old file-browser behavior.

use std::path::PathBuf;

use storey_shared::{Client, settings::Settings};

use crate::progress::Pending;
use crate::{
    detail::TagState, error::ErrorPopup, files::FilesState, fs, plugins_view::PluginsState, tagbar,
};

/// Which screen is shown: plain browser, repo files, tagging, plugins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Browse,
    Files,
    Detail,
    Plugins,
}

/// One visible row: non-hidden file or dir, dirs first, sorted.
#[derive(Debug, Clone)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
}

/// Browse application state.
pub struct App {
    pub client: Client,
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub selected: usize,
    pub mode: Mode,
    pub is_repo: bool,
    /// Absolute library root from `status` (empty outside a library).
    pub root: String,
    pub view: Vec<String>,
    pub categories: Vec<String>,
    pub notice: String,
    pub bar_width: u16,
    /// Last drawn terminal size (for dialog click mapping).
    pub term_w: u16,
    pub term_h: u16,
    /// Focused breadcrumb chip (`view.len()` = `[Add +]`).
    pub tag_sel: usize,
    /// Add-tag dialog visibility and cursor.
    pub add_open: bool,
    pub add_sel: usize,
    /// Repo files (`storey list`) plus cursor for the Files view.
    pub files: FilesState,
    /// Tagging buffers and suggestion popup for the Detail view.
    pub tagging: TagState,
    /// Declared-plugin rows for the Plugins view.
    pub plugins: PluginsState,
    /// `ui.toml` settings (plugin list, binary override).
    pub settings: Settings,
    /// Running background op (`update`/`init`/`restore`), if any.
    pub pending: Option<Pending>,
    /// In-flight progress ticks for the control bar.
    pub progress: Option<crate::progress::Progress>,
    /// Surfaced `storey` failure (modal); ack suppresses re-pops.
    pub error: Option<ErrorPopup>,
    pub(crate) error_ack: bool,
    quit: bool,
}

impl App {
    /// Create the app at `start`; [`refresh`] loads the first view.
    pub fn new(storey_bin: impl Into<PathBuf>, start: PathBuf) -> Self {
        let client = Client::new(storey_bin);
        let mut app = Self {
            client,
            cwd: start,
            entries: Vec::new(),
            selected: 0,
            mode: Mode::Browse,
            is_repo: false,
            root: String::new(),
            view: Vec::new(),
            categories: Vec::new(),
            notice: String::new(),
            bar_width: 0,
            term_w: 0,
            term_h: 0,
            tag_sel: 0,
            add_open: false,
            add_sel: 0,
            files: FilesState::default(),
            tagging: TagState::default(),
            plugins: PluginsState::default(),
            settings: storey_shared::settings::load().unwrap_or_default(),
            pending: None,
            progress: None,
            error: None,
            error_ack: false,
            quit: false,
        };
        app.refresh();
        app
    }

    pub fn quit(&mut self) {
        self.quit = true;
    }
    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    /// Re-read cwd; `status` decides repo (exit 2 = plain dir).
    pub fn refresh(&mut self) {
        self.cwd = fs::canonical(&self.cwd);
        self.entries = fs::list_dir(&self.cwd);
        self.selected = self.selected.min(self.entries.len().saturating_sub(1));
        match self.client.status(&self.cwd, |_| {}) {
            Ok(s) => {
                self.is_repo = true;
                self.view = s.view;
                self.root = s.root;
                match self.client.categories(&self.cwd, |_| {}) {
                    Ok(c) => self.categories = c,
                    Err(e) => self.fail("tag get", &e),
                }
            }
            Err(e) => {
                self.is_repo = false;
                self.view.clear();
                self.root.clear();
                self.categories.clear();
                if e.code != Some(2) {
                    self.fail("status", &e);
                }
            }
        }
        self.tag_sel = self
            .tag_sel
            .min(tagbar::chip_labels(&self.view).len().saturating_sub(1));
        let avail = self.available_tags().len();
        self.add_sel = self.add_sel.min(avail.saturating_sub(1));
        if avail == 0 {
            self.add_open = false;
        }
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    /// Go up. Always allowed — this is how you leave a library.
    /// Descending is what libraries forbid (see [`enter_selected`]).
    pub fn up(&mut self) {
        if let Some(parent) = self.cwd.parent() {
            self.cwd = parent.to_path_buf();
        }
        self.refresh();
    }

    /// Plain dirs only: library subfolders are not browsable.
    pub fn enter_selected(&mut self) {
        if self.is_repo {
            return;
        }
        let Some(entry) = self.selected_entry().cloned() else {
            return;
        };
        if entry.is_dir {
            self.cwd = entry.path;
            self.refresh();
        }
    }

    /// Categories not already in the view (dialog candidates).
    pub fn available_tags(&self) -> Vec<String> {
        tagbar::available(&self.categories, &self.view)
    }

    /// Open the add dialog when an unused category exists.
    pub fn open_add(&mut self) {
        if !self.is_repo {
            return;
        }
        if self.available_tags().is_empty() {
            self.notice = "no unused category".to_string();
            return;
        }
        self.add_open = true;
        self.add_sel = 0;
    }

    pub fn close_add(&mut self) {
        self.add_open = false;
    }

    /// Append the dialog-selected category to the view.
    pub fn confirm_add(&mut self) {
        let avail = self.available_tags();
        let Some(next) = avail.get(self.add_sel).cloned() else {
            self.close_add();
            return;
        };
        self.close_add();
        let mut by = self.view.clone();
        by.push(next);
        self.apply_view(by);
    }

    /// Move breadcrumb focus, wrapping over tags, `[Add +]`, and
    /// `[Restore]` when the view is empty.
    pub fn move_tag(&mut self, delta: i32) {
        let n = tagbar::chip_labels(&self.view).len();
        if n == 0 {
            return;
        }
        let cur = self.tag_sel as i32;
        let next = (cur + delta).rem_euclid(n as i32);
        self.tag_sel = next as usize;
    }

    /// Move the add-dialog cursor, wrapping.
    pub fn move_add_sel(&mut self, delta: i32) {
        let n = self.available_tags().len();
        if n == 0 {
            return;
        }
        let cur = self.add_sel as i32;
        self.add_sel = (cur + delta).rem_euclid(n as i32) as usize;
    }

    /// Activate a breadcrumb: tag removes itself, add opens the dialog,
    /// restore moves stored objects back to their original paths.
    pub fn activate_tag(&mut self, idx: usize) {
        if tagbar::is_restore_chip(&self.view, idx) {
            self.spawn_restore();
        } else if idx >= self.view.len() {
            self.open_add();
        } else {
            self.remove_tag_at(idx);
        }
    }

    /// Activate the focused breadcrumb chip.
    pub fn activate_selected_tag(&mut self) {
        let idx = self.tag_sel;
        self.activate_tag(idx);
    }

    /// Drop one view tag by index, then materialize.
    pub fn remove_tag_at(&mut self, idx: usize) {
        if idx >= self.view.len() {
            return;
        }
        let mut by = self.view.clone();
        by.remove(idx);
        self.apply_view(by);
    }

    /// Drop the focused tag (no-op on `[Add +]`).
    pub fn remove_selected_tag(&mut self) {
        if self.tag_sel < self.view.len() {
            self.remove_tag_at(self.tag_sel);
        }
    }

    /// Run `view --tags=...` (empty `--tags=` renders an empty dir)
    /// and refresh. Removing the last tag clears the view this way.
    fn apply_view(&mut self, by: Vec<String>) {
        self.unack_error();
        let arg;
        let args: &[&str] = if by.is_empty() {
            &["view", "--tags="]
        } else {
            arg = format!("--tags={}", by.join(","));
            &["view", &arg]
        };
        match self.client.call(args, &self.cwd, |_| {}) {
            Ok(_) => self.notice = String::new(),
            Err(e) => self.fail("view", &e),
        }
        self.refresh();
    }

    /// Click on a file-list row (plain dirs only).
    pub fn click_row(&mut self, row: usize) {
        if self.is_repo {
            return;
        }
        if row < self.entries.len() {
            self.selected = row;
            self.enter_selected();
        }
    }
}
