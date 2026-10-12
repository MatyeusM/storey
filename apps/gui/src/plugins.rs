//! Plugin dialog invokables plus Qt-free row builders.
//!
//! Rows show plugin health; adding a plugin shallow-
//! clones the git url into the config plugins dir and records it
//! in `ui.toml`.

use core::pin::Pin;
use std::path::PathBuf;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use storey_shared::tags::TagSource;

use crate::{store::ffi, support};

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

impl ffi::StoreyLibrary {
    pub(crate) fn reload_plugins(mut self: Pin<&mut Self>) {
        let settings = storey_shared::settings::load().unwrap_or_default();
        let rows: Vec<PluginRow> = settings.plugins.iter().map(build_row).collect();
        self.as_mut().rust_mut().settings = settings;
        self.as_mut().rust_mut().plugin_rows = rows;
        let json = {
            let rust = self.rust();
            plugins_json(&rust.plugin_rows)
        };
        self.as_mut().set_plugins_json(support::qstr(&json));
    }

    pub(crate) fn set_plugin_enabled(mut self: Pin<&mut Self>, index: i32, enabled: bool) {
        let mut settings = self.rust().settings.clone();
        let Some(decl) = settings.plugins.get_mut(index as usize) else {
            return;
        };
        decl.enabled = enabled;
        if save_settings(&settings) {
            self.as_mut().rust_mut().settings = settings;
            self.as_mut().reload_plugins();
        } else {
            fail_status(&mut self, "cannot write ui.toml");
        }
    }

    pub(crate) fn add_plugin(mut self: Pin<&mut Self>, url: QString) {
        let url = support::s(&url).trim().to_string();
        if url.is_empty() {
            fail_status(&mut self, "enter a git url");
            return;
        }
        let name = name_from_git_url(&url);
        let Some(dir) = plugin_dir(&name) else {
            fail_status(&mut self, "cannot locate config dir");
            return;
        };
        if dir.exists() {
            fail_status(&mut self, "plugin already exists");
            return;
        }
        match clone_plugin(&url, &dir) {
            Ok(()) => {
                let mut settings = self.rust().settings.clone();
                settings.plugins.push(storey_shared::settings::PluginDecl {
                    name,
                    path: dir,
                    enabled: true,
                    extensions: Vec::new(),
                    timeout_secs: 10,
                });
                if save_settings(&settings) {
                    self.as_mut().rust_mut().settings = settings;
                    self.as_mut().reload_plugins();
                    self.as_mut().set_status_text(support::qstr(""));
                } else {
                    fail_status(&mut self, "cannot write ui.toml");
                }
            }
            Err(detail) => fail_status(&mut self, &detail),
        }
    }
}

/// Plugin rows serialized for the plugins dialog.
pub(crate) fn plugins_json(rows: &[PluginRow]) -> String {
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
pub(crate) fn name_from_git_url(url: &str) -> String {
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
pub(crate) fn build_row(decl: &storey_shared::settings::PluginDecl) -> PluginRow {
    let version = plugin_version(&decl.path);
    let enabled = decl.enabled;
    match storey_shared::tags::ExternalPlugin::from_decl(decl) {
        Ok(plugin) if plugin.available() => row(decl, version, enabled, true, "ok"),
        Ok(_) => row(decl, version, enabled, false, "missing binary"),
        Err(e) => row(
            decl,
            version,
            enabled,
            false,
            &storey_shared::explore::short_err(e.to_string()),
        ),
    }
}

/// Row constructor for [`build_row`].
fn row(
    decl: &storey_shared::settings::PluginDecl,
    version: String,
    enabled: bool,
    ok: bool,
    status: &str,
) -> PluginRow {
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
        match storey_shared::manifest::load_manifest(path) {
            Ok(m) if !m.version.is_empty() => m.version,
            _ => "?".to_string(),
        }
    } else {
        "-".to_string()
    }
}

/// Config-dir plugin folder for `name`.
fn plugin_dir(name: &str) -> Option<PathBuf> {
    let base = storey_shared::settings::config_dir()?;
    Some(base.join("storey").join("plugins").join(name))
}

/// Persist settings to the config path.
fn save_settings(settings: &storey_shared::settings::Settings) -> bool {
    let Some(path) = storey_shared::settings::config_path() else {
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
    let short = storey_shared::explore::short_err(tail.to_string());
    Err(if short.is_empty() {
        "git clone failed".to_string()
    } else {
        short
    })
}

/// Set a `storey error:` status line.
fn fail_status(obj: &mut Pin<&mut ffi::StoreyLibrary>, detail: &str) {
    obj.as_mut()
        .set_status_text(support::qstr(&format!("storey error: {detail}")));
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
        assert!(!row.status.is_empty());
        let rows: serde_json::Value =
            serde_json::from_str(&plugins_json(std::slice::from_ref(&row))).unwrap();
        assert_eq!(rows[0]["name"], "bad");
    }
}
