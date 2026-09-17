//! Plugins view: rows from `ui.toml` declarations.
//!
//! Each row shows name, version, path, and ok/broken health via
//! [`ExternalPlugin::from_decl`] plus [`TagSource::available`].
//! `Enter` opens the plugin folder in the plain Browse browser:
//! the dir itself, or the parent dir for file plugins.

use std::path::PathBuf;

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};
use storey_shared::tags::{ExternalPlugin, TagSource};

use crate::{app::App, fs};

/// One declared plugin with health info.
#[derive(Debug, Clone)]
pub struct PluginRow {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub ok: bool,
    pub status: String,
}

/// Cursor over the loaded plugin rows.
#[derive(Debug, Default)]
pub struct PluginsState {
    pub rows: Vec<PluginRow>,
    pub sel: usize,
}

impl PluginsState {
    fn clamp(&mut self) {
        self.sel = self.sel.min(self.rows.len().saturating_sub(1));
    }
}

/// Build one row: version from `manifest.json` dirs, health from decl.
fn build_row(decl: &storey_shared::settings::PluginDecl) -> PluginRow {
    let version = plugin_version(&decl.path);
    match ExternalPlugin::from_decl(decl) {
        Ok(plugin) if plugin.available() => PluginRow {
            name: decl.name.clone(),
            version,
            path: decl.path.clone(),
            ok: true,
            status: "ok".to_string(),
        },
        Ok(_) => PluginRow {
            name: decl.name.clone(),
            version,
            path: decl.path.clone(),
            ok: false,
            status: "missing binary".to_string(),
        },
        Err(e) => PluginRow {
            name: decl.name.clone(),
            version,
            path: decl.path.clone(),
            ok: false,
            status: fs::short_err(e.to_string()),
        },
    }
}

/// Version for dir plugins via `manifest.json`, `-` otherwise.
fn plugin_version(path: &std::path::Path) -> String {
    if path.is_dir() {
        match storey_shared::manifest::load_manifest(path) {
            Ok(m) if !m.version.is_empty() => m.version,
            _ => "?".to_string(),
        }
    } else {
        "-".to_string()
    }
}

impl App {
    /// Reload rows from the current settings declarations.
    pub fn reload_plugins(&mut self) {
        self.plugins.rows = self.settings.plugins.iter().map(build_row).collect();
        self.plugins.clamp();
    }

    /// Move the plugins cursor, clamped.
    pub fn move_plugins(&mut self, delta: i32) {
        if self.plugins.rows.is_empty() {
            return;
        }
        let len = self.plugins.rows.len() as i32;
        let next = (self.plugins.sel as i32 + delta).clamp(0, len - 1);
        self.plugins.sel = next as usize;
    }

    /// Open the selected plugin's folder in the plain Browse browser.
    pub fn open_plugin_folder(&mut self) {
        let Some(row) = self.plugins.rows.get(self.plugins.sel).cloned() else {
            return;
        };
        let dir = if row.path.is_dir() {
            row.path.clone()
        } else {
            row.path
                .parent()
                .map(|p| p.to_path_buf())
                .unwrap_or(row.path.clone())
        };
        self.cwd = fs::canonical(&dir);
        self.mode = crate::app::Mode::Browse;
        self.refresh();
    }

    /// Keys for the plugins view; true quits.
    pub fn on_key_plugins(
        &mut self,
        code: crossterm::event::KeyCode,
        mods: crossterm::event::KeyModifiers,
    ) -> bool {
        use crossterm::event::{KeyCode, KeyModifiers};
        if code == KeyCode::Char('q') && mods == KeyModifiers::NONE {
            return true;
        }
        match code {
            KeyCode::Down | KeyCode::Char('j') => self.move_plugins(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_plugins(-1),
            KeyCode::Enter => self.open_plugin_folder(),
            KeyCode::Esc => self.mode = crate::app::Mode::Detail,
            _ => {}
        }
        false
    }
}

/// Plugin table: `name version path [status]`, selected row cyan.
pub fn render_plugins(app: &App) -> List<'_> {
    let items: Vec<ListItem<'_>> = app
        .plugins
        .rows
        .iter()
        .enumerate()
        .map(|(i, row)| ListItem::new(plugin_line(row, i == app.plugins.sel)))
        .collect();
    let title = format!("plugins ({})", app.plugins.rows.len());
    if items.is_empty() {
        List::new(vec![ListItem::new(
            "no plugins — add [[plugins]] to ui.toml",
        )])
        .block(Block::default().borders(Borders::ALL).title(title))
    } else {
        List::new(items).block(Block::default().borders(Borders::ALL).title(title))
    }
}

/// One plugin row with health suffix; broken rows render red.
fn plugin_line(row: &PluginRow, selected: bool) -> Line<'static> {
    let text = format!(
        "{} {} {} [{}]",
        row.name,
        row.version,
        row.path.display(),
        row.status
    );
    if selected {
        Line::from(vec![
            Span::styled("> ", Style::default()),
            Span::styled(text, Style::default().fg(Color::Cyan)),
        ])
    } else if row.ok {
        Line::from(vec![Span::raw("  "), Span::raw(text)])
    } else {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(text, Style::default().fg(Color::Red)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, Mode};

    fn plugins_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-tui-plugins-{}", std::process::id()));
        let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
        app.close_error();
        app.mode = Mode::Plugins;
        app
    }

    #[test]
    fn empty_rows_hint_and_clamp() {
        let mut app = plugins_app();
        app.reload_plugins();
        assert!(app.plugins.rows.is_empty());
        app.move_plugins(1);
        assert_eq!(app.plugins.sel, 0);
    }

    #[test]
    fn broken_path_marks_row() {
        let decl = storey_shared::settings::PluginDecl {
            name: "bad".into(),
            path: PathBuf::from("/no-such-plugin-xyz"),
            enabled: true,
            extensions: Vec::new(),
            timeout_secs: 10,
        };
        let row = build_row(&decl);
        assert!(!row.ok);
        assert_eq!(row.version, "-");
    }

    #[test]
    fn esc_returns_to_detail() {
        use crossterm::event::{KeyCode, KeyModifiers};
        let mut app = plugins_app();
        app.on_key_plugins(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(app.mode, Mode::Detail);
    }
}
