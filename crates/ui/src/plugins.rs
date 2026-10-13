//! Plugin dialog state plus Qt-free row builders.
//!
//! Rows show plugin health; adding a plugin shallow-clones the git
//! url into the config plugins dir and records it in `ui.toml`.

use std::path::PathBuf;

use crate::{settings::PluginDecl, state::State, support, tags::TagSource};

/// One declared plugin with health info.
#[derive(Debug, Clone)]
pub struct PluginRow {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub enabled: bool,
    pub ok: bool,
    pub status: String,
}

impl State {
    /// Re-read `ui.toml` and rebuild the health rows.
    pub fn reload_plugins(&mut self) {
        self.settings = crate::settings::load().unwrap_or_default();
        self.plugin_rows = self.settings.plugins.iter().map(build_row).collect();
        self.plugins_json = plugins_json(&self.plugin_rows);
    }

    /// Toggle one declaration; unknown index is silent.
    pub fn set_plugin_enabled(&mut self, index: usize, enabled: bool) {
        let Some(decl) = self.settings.plugins.get_mut(index) else {
            return;
        };
        decl.enabled = enabled;
        if save_settings(&self.settings) {
            self.reload_plugins();
        } else {
            self.status_text = "storey error: cannot write ui.toml".into();
        }
    }

    /// Shallow-clone a git url into the config plugins dir.
    pub fn add_plugin(&mut self, url: &str) {
        let url = url.trim().to_string();
        if url.is_empty() {
            self.status_text = "storey error: enter a git url".into();
            return;
        }
        let name = name_from_git_url(&url);
        let Some(dir) = plugin_dir(&name) else {
            self.status_text = "storey error: cannot locate config dir".into();
            return;
        };
        if dir.exists() {
            self.status_text = "storey error: plugin already exists".into();
            return;
        }
        match clone_plugin(&url, &dir) {
            Ok(()) => {
                self.settings.plugins.push(PluginDecl {
                    name,
                    path: dir,
                    enabled: true,
                    extensions: Vec::new(),
                    timeout_secs: 10,
                });
                if save_settings(&self.settings) {
                    self.reload_plugins();
                    self.status_text.clear();
                } else {
                    self.status_text = "storey error: cannot write ui.toml".into();
                }
            }
            Err(detail) => self.status_text = format!("storey error: {detail}"),
        }
    }
}

/// Plugin rows serialized for the plugins dialog.
pub fn plugins_json(rows: &[PluginRow]) -> String {
    let out: Vec<serde_json::Value> = rows
        .iter()
        .map(|row| {
            serde_json::json!({
                "name": row.name,
                "version": row.version,
                "path": row.path.display().to_string(),
                "enabled": row.enabled,
                "ok": row.ok,
                "status": row.status,
            })
        })
        .collect();
    serde_json::to_string(&out).unwrap_or_else(|_| "[]".into())
}

/// Plugin name from a git url (`…/fx.git` → `fx`).
pub fn name_from_git_url(url: &str) -> String {
    let tail = url
        .trim()
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("");
    let tail = tail.trim_end_matches(".git");
    if tail.is_empty() {
        "plugin".to_string()
    } else {
        tail.to_string()
    }
}

/// Build one plugin row: manifest version plus binary health.
pub fn build_row(decl: &PluginDecl) -> PluginRow {
    let version = plugin_version(&decl.path);
    let enabled = decl.enabled;
    match crate::tags::ExternalPlugin::from_decl(decl) {
        Ok(plugin) if plugin.available() => row(decl, version, enabled, true, "ok"),
        Ok(_) => row(decl, version, enabled, false, "missing binary"),
        Err(e) => row(
            decl,
            version,
            enabled,
            false,
            &support::short_detail(&e.to_string()),
        ),
    }
}

/// Row constructor for [`build_row`].
fn row(decl: &PluginDecl, version: String, enabled: bool, ok: bool, status: &str) -> PluginRow {
    PluginRow {
        name: decl.name.clone(),
        version,
        path: decl.path.clone(),
        enabled,
        ok,
        status: status.to_string(),
    }
}

/// Version for dir plugins via `manifest.json`, `-` otherwise.
fn plugin_version(path: &std::path::Path) -> String {
    if path.is_dir() {
        match crate::manifest::load_manifest(path) {
            Ok(m) if !m.version.is_empty() => m.version,
            _ => "?".to_string(),
        }
    } else {
        "-".to_string()
    }
}

/// Config-dir plugin folder for `name`.
fn plugin_dir(name: &str) -> Option<PathBuf> {
    let base = crate::settings::config_dir()?;
    Some(base.join("storey").join("plugins").join(name))
}

/// Persist settings to the config path.
fn save_settings(settings: &crate::settings::Settings) -> bool {
    let Some(path) = crate::settings::config_path() else {
        return false;
    };
    settings.save_to(&path).is_ok()
}

/// Shallow-clone `url` into `dir`; Ok or a short status detail.
fn clone_plugin(url: &str, dir: &std::path::Path) -> Result<(), String> {
    if let Some(parent) = dir.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let out = std::process::Command::new("git")
        .args(["clone", "--depth", "1", url])
        .arg(dir)
        .output()
        .map_err(|e| format!("git not found ({e})"))?;
    if out.status.success() {
        return Ok(());
    }
    let tail = String::from_utf8_lossy(&out.stderr);
    let short = support::short_detail(tail.as_ref());
    Err(if short.is_empty() {
        "git clone failed".to_string()
    } else {
        short
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_url_names_fall_back() {
        assert_eq!(name_from_git_url("https://x/fx.git"), "fx");
        assert_eq!(name_from_git_url("https://x/fx/"), "fx");
        assert_eq!(name_from_git_url(""), "plugin");
    }

    #[test]
    fn broken_plugin_marks_row() {
        let decl = PluginDecl {
            name: "bad".into(),
            path: PathBuf::from("/no-such-plugin-xyz"),
            enabled: true,
            extensions: Vec::new(),
            timeout_secs: 10,
        };
        let row = build_row(&decl);
        assert!(!row.ok);
        assert_eq!(row.version, "-");
        assert!(!row.status.is_empty());
        let rows: serde_json::Value =
            serde_json::from_str(&plugins_json(std::slice::from_ref(&row))).unwrap();
        assert_eq!(rows[0]["name"], "bad");
    }
}
