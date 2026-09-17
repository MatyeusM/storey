//! Locate the `storey` CLI binary.
//!
//! Order: `--storey-bin`, a custom `storey_bin` in `ui.toml`, a
//! `storey` shipped next to this executable, then `storey` on PATH.

use std::path::{Path, PathBuf};

/// Resolve which `storey` binary to drive.
///
/// Order: `--storey-bin`, a custom `storey_bin` in `ui.toml`, a
/// `storey` shipped next to this executable, then `storey` on PATH.
/// Relative results are anchored at the process cwd: CLI calls run
/// with `current_dir` set to the open folder, so a bare relative
/// path would break on navigation (see `storey-shared::binpath`).
pub fn resolve(explicit: Option<&str>) -> PathBuf {
    let bin = if let Some(bin) = explicit.filter(|s| !s.is_empty()) {
        PathBuf::from(bin)
    } else if let Some(bin) = configured_custom() {
        bin
    } else {
        sibling().unwrap_or_else(default_bin)
    };
    match std::env::current_dir() {
        Ok(cwd) => storey_shared::binpath::absolutize(&bin, &cwd),
        Err(_) => bin,
    }
}

/// Notice when the binary cannot even be spawned.
pub fn missing_notice(bin: &Path) -> String {
    format!(
        "no storey binary at {} (--storey-bin to override)",
        bin.display()
    )
}

fn default_bin() -> PathBuf {
    configured().unwrap_or_else(|| PathBuf::from("storey"))
}

/// `storey_bin` from `ui.toml`, when the file exists and parses.
fn configured() -> Option<PathBuf> {
    let path = storey_shared::settings::config_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let settings = storey_shared::settings::parse(&text).ok()?;
    let bin = settings.storey_bin.trim().to_string();
    if bin.is_empty() {
        None
    } else {
        Some(PathBuf::from(bin))
    }
}

/// `ui.toml` value when it differs from the PATH default.
fn configured_custom() -> Option<PathBuf> {
    let bin = configured()?;
    if bin.as_os_str() == "storey" {
        None
    } else {
        Some(bin)
    }
}

/// A `storey` shipped next to this executable (release installs).
fn sibling() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let cand = dir.join(sibling_name());
    if cand.is_file() { Some(cand) } else { None }
}

#[cfg(windows)]
fn sibling_name() -> &'static str {
    "storey.exe"
}

#[cfg(not(windows))]
fn sibling_name() -> &'static str {
    "storey"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_flag_wins() {
        assert_eq!(resolve(Some("/opt/storey")), PathBuf::from("/opt/storey"));
    }

    #[test]
    fn sibling_lookup_never_panics() {
        let _ = sibling();
    }

    #[test]
    fn notice_names_binary() {
        assert!(missing_notice(Path::new("storey")).contains("storey"));
    }

    #[test]
    fn removing_last_view_tag_empties_dir() {
        let bin = std::env::var("STOREY_BIN").unwrap_or_default();
        if !std::path::Path::new(&bin).is_file() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("storey-e2e-last-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        for args in [
            vec!["init"],
            vec![
                "tag",
                "set",
                "a.txt",
                "--category",
                "genre",
                "--values",
                "rock",
            ],
            vec!["view", "--tags=genre"],
        ] {
            let out = std::process::Command::new(&bin)
                .args(&args)
                .current_dir(&dir)
                .output()
                .unwrap();
            assert!(out.status.success(), "{args:?} failed");
        }
        let mut app = crate::app::App::new(std::path::PathBuf::from(bin), dir.clone());
        assert_eq!(app.view, vec!["genre".to_string()]);
        app.remove_tag_at(0);
        assert!(app.view.is_empty(), "notice was {:?}", app.notice);
        assert!(!dir.join("rock").exists());
        assert!(dir.join(".files/a.txt").is_file());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn provided_binary_detects_real_repo() {
        let bin = std::env::var("STOREY_BIN").unwrap_or_default();
        if !std::path::Path::new(&bin).is_file() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("storey-e2e-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        let init = std::process::Command::new(&bin)
            .arg("init")
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(init.status.success());
        let app = crate::app::App::new(std::path::PathBuf::from(bin), dir.clone());
        assert!(app.is_repo, "notice was {:?}", app.notice);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
