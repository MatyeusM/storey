//! Plugin `manifest.json`: self-description for directory plugins.
//!
//! A `[[plugins]] path` in `ui.toml` may point at a single executable
//! (run directly) or at a directory holding `manifest.json`, which
//! declares how to run it. Directory form lets authors ship an
//! interpreter plus script:
//!
//! ```json
//! {
//!   "name": "music",
//!   "version": "0.1.0",
//!   "description": "acoustic id lookup",
//!   "entrypoint": ["python3", "suggest.py"],
//!   "supported_extensions": ["mp3", "flac"],
//!   "protocol": 1
//! }
//! ```
//!
//! Protocol 1 is record JSON on stdin, suggestion array on stdout.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Name of the manifest file inside a plugin directory.
pub const MANIFEST_FILE: &str = "manifest.json";

fn default_protocol() -> u32 {
    1
}

/// Self-description loaded from `manifest.json`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub entrypoint: Vec<String>,
    #[serde(default)]
    pub supported_extensions: Vec<String>,
    #[serde(default = "default_protocol")]
    pub protocol: u32,
}

/// Resolved command for one plugin: what to spawn and where.
#[derive(Debug, Clone)]
pub struct ResolvedCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

/// Load `manifest.json` from a plugin directory.
pub fn load_manifest(dir: &Path) -> Result<Manifest> {
    let path = dir.join(MANIFEST_FILE);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let m: Manifest =
        serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    if m.entrypoint.is_empty() {
        anyhow::bail!("{} has empty entrypoint", path.display());
    }
    if m.protocol != 1 {
        anyhow::bail!("unsupported plugin protocol {}", m.protocol);
    }
    Ok(m)
}

/// Resolve a configured path to a runnable command.
/// Files run directly; directories need `manifest.json`.
pub fn resolve(path: &Path) -> Result<ResolvedCommand> {
    if path.is_file() {
        let cwd = path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        return Ok(ResolvedCommand {
            program: path.to_path_buf(),
            args: Vec::new(),
            cwd,
        });
    }
    if path.is_dir() {
        let m = load_manifest(path)?;
        let (program, args) = split_entrypoint(&m.entrypoint, path);
        return Ok(ResolvedCommand {
            program,
            args,
            cwd: path.to_path_buf(),
        });
    }
    anyhow::bail!("plugin path does not exist: {}", path.display())
}

/// Split entrypoint into program plus args; a relative program that
/// names a file inside the plugin dir resolves against it.
fn split_entrypoint(entry: &[String], dir: &Path) -> (PathBuf, Vec<String>) {
    let mut parts = entry.iter();
    let first = parts.next().cloned().unwrap_or_default();
    let candidate = dir.join(&first);
    let program = if first.contains('/') || first.contains('\\') || candidate.is_file() {
        if Path::new(&first).is_absolute() {
            PathBuf::from(first)
        } else if candidate.is_file() {
            candidate
        } else {
            PathBuf::from(first)
        }
    } else {
        PathBuf::from(first)
    };
    (program, parts.cloned().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_protocol() {
        let dir = std::env::temp_dir().join("storey-manifest-proto");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join(MANIFEST_FILE),
            r#"{"name":"x","entrypoint":["run"],"protocol":9}"#,
        )
        .unwrap();
        assert!(load_manifest(&dir).is_err());
    }

    #[test]
    fn missing_manifest_errs() {
        let dir = std::env::temp_dir().join("storey-manifest-missing-dir");
        std::fs::create_dir_all(&dir).unwrap();
        let missing = dir.join("no-such-subdir-xyz");
        assert!(resolve(&missing).is_err());
    }

    #[test]
    fn file_resolves_directly() {
        let dir = std::env::temp_dir();
        let exe = dir.join("storey-manifest-probe-file");
        std::fs::write(&exe, "#!/bin/sh\n").unwrap();
        let r = resolve(&exe).unwrap();
        assert_eq!(r.program, exe);
        assert!(r.args.is_empty());
        let _ = std::fs::remove_file(&exe);
    }
}
