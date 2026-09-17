//! View 2 — file tagging: edit one record's tags by id.
//!
//! [`TagState`] keeps the focused record id, category/value buffers,
//! set/add mode, and the suggestion popup. CLI writes go through the
//! record id; values split on `,` (see [`records`]). After tagging the
//! materialized view is stale on purpose — no auto re-view.

use std::collections::BTreeMap;

use crossterm::event::{KeyCode, KeyModifiers};
use storey_shared::{
    model::Record,
    tags::{ExternalPlugin, Suggestion, TagSource, rank},
};

use crate::{app::App, records};

/// Which text buffer receives keystrokes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Cat,
    Vals,
}

/// Tagging state for one record (see module docs).
#[derive(Debug, Default)]
pub struct TagState {
    pub record_id: String,
    pub cat: String,
    pub vals: String,
    pub focus: Option<Focus>,
    pub set_mode: bool,
    pub suggestions: Vec<Suggestion>,
    pub suggest_open: bool,
}

impl TagState {
    /// Start tagging `id` with fresh buffers in set mode.
    pub fn open(&mut self, id: String) {
        self.record_id = id;
        self.cat.clear();
        self.vals.clear();
        self.focus = None;
        self.set_mode = true;
        self.suggestions.clear();
        self.suggest_open = false;
    }
}

impl App {
    /// Record under edit, looked up by id from the loaded rows.
    pub fn detail_record(&self) -> Option<&Record> {
        records::find_by_id(&self.files.records, &self.tagging.record_id)
    }

    /// Push a printable char into the focused buffer.
    fn push_input(&mut self, ch: char) {
        match self.tagging.focus {
            Some(Focus::Cat) => self.tagging.cat.push(ch),
            Some(Focus::Vals) => self.tagging.vals.push(ch),
            None => {}
        }
    }

    /// Backspace the focused buffer (append-only, no cursor).
    fn pop_input(&mut self) {
        match self.tagging.focus {
            Some(Focus::Cat) => {
                self.tagging.cat.pop();
            }
            Some(Focus::Vals) => {
                self.tagging.vals.pop();
            }
            None => {}
        }
    }

    /// Tab-complete the category buffer from loaded categories.
    fn complete_cat(&mut self) {
        if self.tagging.focus != Some(Focus::Cat) {
            return;
        }
        let cats = records::categories_with_counts(&self.files.records);
        if let Some(full) = records::complete_category(&cats, &self.tagging.cat) {
            self.tagging.cat = full;
        }
    }

    /// Apply the buffers via `tag set`/`tag add` on the record id.
    /// Both buffers clear on success; the record line above re-reads
    /// from the reloaded list, so it shows the applied values.
    pub fn apply_tags(&mut self) {
        self.unack_error();
        let cat = self.tagging.cat.trim().to_string();
        let values = records::split_values(&self.tagging.vals);
        if cat.is_empty() {
            self.notice = "enter a category (c)".to_string();
            return;
        }
        if values.is_empty() {
            self.notice = "enter comma-separated values (v)".to_string();
            return;
        }
        let id = self.tagging.record_id.clone();
        let result = if self.tagging.set_mode {
            self.client.set_tags(&self.cwd, &id, &cat, &values, |_| {})
        } else {
            self.client.add_tags(&self.cwd, &id, &cat, &values, |_| {})
        };
        match result {
            Ok(_) => {
                self.tagging.cat.clear();
                self.tagging.vals.clear();
                self.tagging.focus = None;
                self.load_records();
                self.notice = "tagged (view stale — re-run view to materialize)".to_string();
            }
            Err(e) => self.fail("tag", &e),
        }
    }

    /// Query enabled plugins for the focused record, rank, and open the
    /// accept popup. Rels go out absolute against the library root.
    pub fn do_suggest(&mut self) {
        let Some(record) = self.detail_record().cloned() else {
            return;
        };
        let root = std::path::PathBuf::from(&self.root);
        let all: Vec<_> = self
            .settings
            .enabled_plugins()
            .filter_map(|decl| ExternalPlugin::from_decl(decl).ok())
            .filter_map(|plugin| plugin.lookup(&record, &root).ok())
            .flatten()
            .collect();
        let ranked = rank(all);
        if ranked.is_empty() {
            self.notice = "no suggestions".to_string();
            return;
        }
        self.tagging.suggestions = ranked;
        self.tagging.suggest_open = true;
    }

    /// Apply every suggestion via `tag add`, grouped by category.
    pub fn apply_suggestions(&mut self) {
        self.unack_error();
        let id = self.tagging.record_id.clone();
        let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for s in &self.tagging.suggestions {
            let values = grouped.entry(s.category.clone()).or_default();
            if !values.contains(&s.value) {
                values.push(s.value.clone());
            }
        }
        for (cat, values) in &grouped {
            if let Err(e) = self.client.add_tags(&self.cwd, &id, cat, values, |_| {}) {
                self.fail("tag", &e);
                return;
            }
        }
        self.tagging.suggestions.clear();
        self.tagging.suggest_open = false;
        self.load_records();
        self.notice = "suggestions applied (view stale — re-run view to materialize)".to_string();
    }

    /// Enter the plugins view from Detail.
    pub fn enter_plugins(&mut self) {
        self.reload_plugins();
        self.mode = crate::app::Mode::Plugins;
    }

    /// Keys for the suggestion accept popup; true quits.
    fn on_key_suggest(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        if code == KeyCode::Char('q') && mods == KeyModifiers::NONE {
            return true;
        }
        match code {
            KeyCode::Char('y') | KeyCode::Char('Y') => self.apply_suggestions(),
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.tagging.suggest_open = false;
            }
            _ => {}
        }
        false
    }

    /// Keys while a text buffer is focused (plain text only); true quits (never).
    /// `Ctrl`/`Alt` combos are ignored so `Ctrl-C` cannot type a `c`.
    fn on_key_editing(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        match code {
            KeyCode::Esc => self.tagging.focus = None,
            KeyCode::Enter => self.apply_tags(),
            KeyCode::Backspace => self.pop_input(),
            KeyCode::Tab => self.complete_cat(),
            KeyCode::Char(ch) if mods == KeyModifiers::NONE || mods == KeyModifiers::SHIFT => {
                self.push_input(ch)
            }
            _ => {}
        }
        false
    }

    /// Keys for the Detail view; true quits.
    pub fn on_key_detail(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        if self.tagging.suggest_open {
            return self.on_key_suggest(code, mods);
        }
        if self.tagging.focus.is_some() {
            return self.on_key_editing(code, mods);
        }
        if code == KeyCode::Char('q') && mods == KeyModifiers::NONE {
            return true;
        }
        match code {
            KeyCode::Char('c') => self.tagging.focus = Some(Focus::Cat),
            KeyCode::Char('v') => self.tagging.focus = Some(Focus::Vals),
            KeyCode::Char('m') => self.tagging.set_mode = !self.tagging.set_mode,
            KeyCode::Enter => self.apply_tags(),
            KeyCode::Char('s') => self.do_suggest(),
            KeyCode::Char('p') | KeyCode::Char('P') => self.enter_plugins(),
            KeyCode::Char('x') | KeyCode::Char('X') => self.remove_category(),
            KeyCode::Esc => self.mode = crate::app::Mode::Files,
            _ => {}
        }
        false
    }
}

#[cfg(test)]
#[path = "detail_tests.rs"]
mod tests;
