//! Shared UI settings: OS config dir plus `storey/ui.toml`.
//!
//! The file is optional; missing means defaults. Plugins are an
//! ordered `[[plugins]]` array so each entry can carry its own path,
//! enable flag, extension filter, and timeout:
//!
//! ```toml
//! storey_bin = "storey"
//! [gui]
//! rem_px = 18
//! [[plugins]]
//! name = "music"
//! path = "/home/u/bin/storey-suggest-music"
//! enabled = true
//! extensions = ["mp3", "flac"]
//! timeout_secs = 10
//! ```

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

fn default_bin() -> String {
    "storey".to_string()
}

fn default_enabled() -> bool {
    true
}

fn default_timeout() -> u64 {
    10
}

/// One plugin entry in `ui.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDecl {
    pub name: String,
    pub path: PathBuf,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub extensions: Vec<String>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

/// Whole `ui.toml` document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default = "default_bin")]
    pub storey_bin: String,
    #[serde(default)]
    pub plugins: Vec<PluginDecl>,
    /// `[gui]` design-token overrides (colors, base font size).
    #[serde(default)]
    pub gui: crate::theme::GuiOverrides,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            storey_bin: default_bin(),
            plugins: Vec::new(),
            gui: crate::theme::GuiOverrides::default(),
        }
    }
}

/// OS config base: `APPDATA` on Windows, Library on macOS, XDG on Linux.
pub fn config_dir() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(PathBuf::from)
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join("Library").join("Application Support"))
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Some(xdg) = std::env::var_os("XDG_CONFIG_HOME") {
            return Some(PathBuf::from(xdg));
        }
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|h| h.join(".config"))
    }
}

/// Full path to `storey/ui.toml`, or None when no home is known.
pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|d| d.join("storey").join("ui.toml"))
}

/// Load settings; missing file yields defaults.
pub fn load() -> Result<Settings> {
    let Some(path) = config_path() else {
        return Ok(Settings::default());
    };
    load_from(&path)
}

/// Load from an explicit path; missing file yields defaults.
pub fn load_from(path: &std::path::Path) -> Result<Settings> {
    match std::fs::read_to_string(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Settings::default()),
        Err(e) => Err(e).with_context(|| format!("reading {}", path.display())),
        Ok(text) => parse(&text),
    }
}

/// Parse `ui.toml` text.
pub fn parse(text: &str) -> Result<Settings> {
    toml::from_str(text).context("parsing ui.toml")
}

impl Settings {
    /// Enabled plugins in declared order.
    pub fn enabled_plugins(&self) -> impl Iterator<Item = &PluginDecl> {
        self.plugins.iter().filter(|p| p.enabled)
    }

    /// Write settings to `path`, creating parent dirs.
    pub fn save_to(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        let text = toml::to_string_pretty(self).context("encoding ui.toml")?;
        std::fs::write(path, text).with_context(|| format!("writing {}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_when_empty() {
        let s: Settings = parse("").unwrap();
        assert_eq!(s.storey_bin, "storey");
        assert!(s.plugins.is_empty());
    }

    #[test]
    fn parses_plugin_array() {
        let s = parse(
            "[[plugins]]\nname = \"music\"\npath = \"/bin/x\"\n"
                .to_owned()
                .as_str(),
        )
        .unwrap();
        assert_eq!(s.plugins.len(), 1);
        assert!(s.plugins[0].enabled);
        assert_eq!(s.plugins[0].timeout_secs, 10);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = std::env::temp_dir().join("storey-no-such-file-ui-toml");
        let s = load_from(&dir.join("ui.toml")).unwrap();
        assert!(s.plugins.is_empty());
    }

    #[test]
    fn parses_gui_overrides() {
        let s = parse("[gui]\nrem_px = 18\ndanger = \"#ff00ff\"\n").unwrap();
        let theme = crate::theme::GuiTheme::resolve(&s.gui);
        assert_eq!(theme.rem_px, 18.0);
        assert_eq!(theme.danger, "#ff00ff");
        assert_eq!(theme.background, crate::theme::BACKGROUND);
    }

    #[test]
    fn disabled_plugins_filtered() {
        let text = "[[plugins]]\nname = \"a\"\npath = \"/a\"\nenabled = false\n".to_owned()
            + "[[plugins]]\nname = \"b\"\npath = \"/b\"\n";
        let s = parse(&text).unwrap();
        let names: Vec<_> = s.enabled_plugins().map(|p| p.name.as_str()).collect();
        assert_eq!(names, vec!["b"]);
    }
}
