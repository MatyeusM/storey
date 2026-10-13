//! UI state plus synchronous sync logic.
//!
//! [`State`] mirrors every QML property in plain data; `apps/gui`
//! copies to and from QStrings around each action. [`sync_all`]
//! re-reads places, explorer rows, `status`, categories, and records
//! after every navigation or mutation.

use std::path::{Path, PathBuf};

use storey_core::flows::UpdateReport;
use storey_core::index::Record;

use crate::{places, settings::Settings, support, tags::Suggestion};

/// Plain-data mirror of the QML explorer state.
pub struct State {
    pub gui_overrides: String,
    pub cwd: String,
    pub root: String,
    pub status_text: String,
    pub entries_json: String,
    pub records_json: String,
    pub view_json: String,
    pub categories_json: String,
    pub detail_json: String,
    pub suggestions_json: String,
    pub plugins_json: String,
    pub places_json: String,
    pub filter_query: String,
    pub selected_id: String,
    pub progress_label: String,
    pub size_text: String,
    pub is_repo: bool,
    pub busy: bool,
    pub file_view: bool,
    pub untagged_only: bool,
    pub folder_count: i32,
    pub file_count: i32,
    pub progress_fraction: f64,
    pub records: Vec<Record>,
    pub view: Vec<String>,
    pub categories: Vec<String>,
    pub suggestions: Vec<Suggestion>,
    pub plugin_rows: Vec<crate::plugins::PluginRow>,
    pub settings: Settings,
}

impl Default for State {
    /// Empty placeholder (never observed; actions replace it).
    fn default() -> Self {
        Self::new(String::new(), Settings::default())
    }
}

impl State {
    /// Fresh state; the shell calls `sync_all` next.
    pub fn new(cwd: String, settings: Settings) -> Self {
        let json = crate::theme::GuiTheme::overrides_json(&settings.gui);
        Self {
            gui_overrides: json.to_string(),
            cwd,
            root: String::new(),
            status_text: String::new(),
            entries_json: "[]".into(),
            records_json: "[]".into(),
            view_json: "[]".into(),
            categories_json: "[]".into(),
            detail_json: "null".into(),
            suggestions_json: "[]".into(),
            plugins_json: "[]".into(),
            places_json: "[]".into(),
            filter_query: String::new(),
            selected_id: String::new(),
            progress_label: String::new(),
            size_text: String::new(),
            is_repo: false,
            busy: false,
            file_view: false,
            untagged_only: false,
            folder_count: 0,
            file_count: 0,
            progress_fraction: -1.0,
            records: Vec::new(),
            view: Vec::new(),
            categories: Vec::new(),
            suggestions: Vec::new(),
            plugin_rows: Vec::new(),
            settings,
        }
    }

    /// Claim the busy flag; refuse with a status line when an op runs.
    pub fn take_busy(&mut self) -> bool {
        if self.busy {
            self.status_text = "storey error: busy — wait for the running op".to_string();
            return true;
        }
        self.busy = true;
        self.progress_label = "indexing…".to_string();
        self.progress_fraction = -1.0;
        false
    }

    /// Apply an `update` outcome, then refresh into the rescanned repo.
    /// Reports carry their problems, so the summary always shows them.
    /// Summary goes last: a plain refresh would clear it.
    pub fn finish_update(&mut self, result: Result<UpdateReport, String>) {
        self.busy = false;
        self.progress_label.clear();
        self.progress_fraction = -1.0;
        sync_all(self);
        match result {
            Ok(report) => self.status_text = report.summary(),
            Err(detail) => support::fail(self, "update", &detail),
        }
    }

    /// Apply an `init` outcome, then refresh into the new repo.
    pub fn finish_init(&mut self, result: Result<(), String>) {
        self.busy = false;
        self.progress_label.clear();
        self.progress_fraction = -1.0;
        match result {
            Ok(()) => self.status_text.clear(),
            Err(detail) => support::fail(self, "init", &detail),
        }
        sync_all(self);
    }
}

/// Re-read places, status, entries, records, and detail.
pub fn sync_all(state: &mut State) {
    sync_cwd(state);
    sync_places(state);
    let cwd = PathBuf::from(state.cwd.clone());
    sync_status(state, &cwd);
    sync_entries_records(state, &cwd);
    sync_detail(state);
}

/// Canonicalize the cwd property in place.
fn sync_cwd(state: &mut State) {
    let raw = PathBuf::from(state.cwd.clone());
    state.cwd = crate::explore::canonical(&raw).display().to_string();
}

/// Refresh the side-pane quick places.
fn sync_places(state: &mut State) {
    state.places_json = places::places_json(&places::quick_places());
}

/// Poll `status` (+ categories) and update repo properties.
fn sync_status(state: &mut State, cwd: &Path) {
    let root = match storey_core::library::find_root(cwd) {
        Ok(root) => root,
        Err(_) => {
            set_not_repo(state);
            return;
        }
    };
    let rep = match storey_core::ops::status_at(&root) {
        Ok(rep) => rep,
        Err(e) => {
            support::fail(state, "status", &support::short_detail(&e.to_string()));
            return;
        }
    };
    state.is_repo = true;
    state.root = rep.root;
    state.view = rep.view.clone();
    state.view_json = support::str_list_json(&rep.view);
    if state.status_text.starts_with("storey error:") {
        return;
    }
    state.status_text.clear();
    match categories_of(&root) {
        Ok(cats) => {
            state.categories = cats.clone();
            state.categories_json = support::str_list_json(&cats);
        }
        Err(detail) => support::fail(state, "status", &detail),
    }
}

/// Distinct categories across the loaded index, sorted.
fn categories_of(root: &Path) -> Result<Vec<String>, String> {
    let ix =
        storey_core::index::Index::load(root).map_err(|e| support::short_detail(&e.to_string()))?;
    Ok(storey_core::tags::categories(&ix.files))
}

/// Clear repo properties for a non-library directory.
fn set_not_repo(state: &mut State) {
    state.is_repo = false;
    state.root.clear();
    state.view.clear();
    state.categories.clear();
    state.view_json = "[]".into();
    state.categories_json = "[]".into();
    if !state.status_text.starts_with("storey error:") {
        state.status_text = "not a storey directory".into();
    }
}

/// Reload `list` rows, repaint filtered models, and recount.
fn sync_entries_records(state: &mut State, cwd: &Path) {
    let query = state.filter_query.clone();
    let untagged = state.untagged_only;
    let is_repo = state.is_repo;
    state.entries_json = support::entries_json(cwd, &query);
    if is_repo {
        let root = storey_core::library::find_root(&PathBuf::from(state.cwd.clone()))
            .unwrap_or_else(|_| PathBuf::from(state.cwd.clone()));
        match storey_core::index::Index::load(&root) {
            Ok(ix) => state.records = ix.files,
            Err(e) => {
                state.records.clear();
                support::fail(state, "list", &support::short_detail(&e.to_string()));
            }
        }
    } else {
        state.records.clear();
    }
    paint_filtered(state, cwd, &query, untagged, is_repo);
}

/// Repaint filtered JSON plus folder/file counts and total size.
fn paint_filtered(state: &mut State, cwd: &Path, query: &str, untagged: bool, is_repo: bool) {
    let entries = crate::explore::list_dir(cwd);
    let q = query.trim().to_lowercase();
    let mut folders = 0i32;
    let mut files = 0i32;
    let mut bytes = 0u64;
    for e in &entries {
        if !e.name.to_lowercase().contains(&q) {
            continue;
        }
        if e.is_dir {
            folders += 1;
        } else if !is_repo {
            files += 1;
            bytes += e.size;
        }
    }
    if is_repo {
        let mut shown = 0i32;
        let mut total = 0i64;
        for r in &state.records {
            let keep = if untagged {
                r.tags.is_empty()
            } else if q.is_empty() {
                true
            } else {
                let in_name = r.display_name.to_lowercase().contains(&q);
                let in_tags = r
                    .tags
                    .values()
                    .flatten()
                    .any(|v| v.to_lowercase().contains(&q))
                    || r.tags.keys().any(|c| c.to_lowercase().contains(&q));
                in_name || in_tags
            };
            if keep {
                shown += 1;
                total += r.size;
            }
        }
        files = shown;
        bytes = total.max(0) as u64;
        state.records_json = support::records_json(&state.records, query, untagged);
    } else {
        state.records_json = "[]".into();
    }
    state.folder_count = folders;
    state.file_count = files;
    state.size_text = places::human_size(bytes);
}

/// Repaint the tag-editor detail for the current selection.
pub fn sync_detail(state: &mut State) {
    state.detail_json = support::detail_json(&state.records, &state.selected_id.clone());
    if !state.records.iter().any(|r| r.id == state.selected_id) {
        state.selected_id.clear();
    }
}
