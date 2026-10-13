//! Tag-suggestion plugins shared by both UIs.
//!
//! There are no built-in suggesters: every source is an external
//! plugin so authors can use any language. [`ExternalPlugin`] speaks
//! the protocol (record JSON on stdin with rels absolutized against
//! the library root, suggestion array on stdout) for either a direct
//! executable or a directory holding `manifest.json`; [`TagSource`] stays as the object-safe trait UIs
//! program against, with [`rank`] to merge results.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::manifest::{self, ResolvedCommand};
use crate::settings::PluginDecl;
use storey_core::index::Record;

/// One suggested `category = value` tag.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Suggestion {
    pub category: String,
    pub value: String,
    /// 0.0..=1.0 confidence, for ranking.
    pub score: f32,
    /// Where the suggestion came from (plugin name).
    pub source: String,
}

/// An external database the UIs can query for tag values.
pub trait TagSource {
    /// Stable source name shown in the UI.
    fn name(&self) -> &str;

    /// True when a lookup can currently run (program present).
    fn available(&self) -> bool;

    /// Suggest tags for a record. Empty is a valid answer.
    ///
    /// `root` is the absolute library root: both rels are sent absolute
    /// so plugins can open the file without knowing the library layout.
    fn lookup(&self, record: &Record, root: &Path) -> Result<Vec<Suggestion>>;
}

/// External suggester plugin from `ui.toml` plus `manifest.json`.
pub struct ExternalPlugin {
    name: String,
    cmd: ResolvedCommand,
    extensions: Vec<String>,
    timeout: Duration,
}

impl ExternalPlugin {
    /// Direct executable, no filter, 10s timeout.
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        let bin = bin.into();
        let cwd = bin
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            name: "external".to_string(),
            cmd: ResolvedCommand {
                program: bin,
                args: Vec::new(),
                cwd,
            },
            extensions: Vec::new(),
            timeout: Duration::from_secs(10),
        }
    }

    /// Build from a `ui.toml` entry, resolving `manifest.json` dirs.
    pub fn from_decl(decl: &PluginDecl) -> Result<Self> {
        let cmd = manifest::resolve(&decl.path)
            .with_context(|| format!("resolving plugin {}", decl.name))?;
        Ok(Self {
            name: decl.name.clone(),
            cmd,
            extensions: decl.extensions.iter().map(|e| lower(e)).collect(),
            timeout: Duration::from_secs(decl.timeout_secs.max(1)),
        })
    }

    /// False when the record's extension is not in the allow-list.
    fn matches(&self, record: &Record) -> bool {
        if self.extensions.is_empty() {
            return true;
        }
        match record.display_name.rsplit('.').next() {
            Some(ext) => self.extensions.iter().any(|e| e == &lower(ext)),
            None => false,
        }
    }
}

impl TagSource for ExternalPlugin {
    fn name(&self) -> &str {
        &self.name
    }

    fn available(&self) -> bool {
        program_present(&self.cmd.program)
    }

    fn lookup(&self, record: &Record, root: &Path) -> Result<Vec<Suggestion>> {
        if !self.matches(record) {
            return Ok(Vec::new());
        }
        let input = serde_json::to_string(&for_plugin(record, root)).context("encoding record")?;
        let mut child = spawn(&self.cmd).context("spawning suggester plugin")?;
        feed(&mut child, input.as_bytes()).context("writing record to plugin")?;
        let out = wait_timeout(child, self.timeout).context("reading plugin")?;
        if !out.status.success() {
            anyhow::bail!("plugin {} exited with {}", self.name, out.status);
        }
        serde_json::from_slice(&out.stdout).context("parsing plugin suggestions")
    }
}

/// Clone `record` with both rels absolutized against the library root,
/// so plugins receive openable paths instead of root-relative ones.
fn for_plugin(record: &Record, root: &Path) -> Record {
    let mut out = record.clone();
    out.original_rel = join_abs(root, &record.original_rel);
    out.stored_rel = join_abs(root, &record.stored_rel);
    out
}

/// `root.join(rel)`, keeping empty rels empty (file not yet stored).
fn join_abs(root: &Path, rel: &str) -> String {
    if rel.is_empty() {
        String::new()
    } else {
        root.join(rel).to_string_lossy().into_owned()
    }
}

fn lower(s: &str) -> String {
    s.to_ascii_lowercase()
}

/// True when the program exists, or is a bare name on PATH.
fn program_present(program: &std::path::Path) -> bool {
    let text = program.to_string_lossy();
    if text.contains('/') || text.contains('\\') {
        program.exists()
    } else {
        true
    }
}

fn spawn(cmd: &ResolvedCommand) -> Result<Child> {
    std::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(&cmd.cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("spawning suggester plugin")
}

fn feed(child: &mut Child, input: &[u8]) -> Result<()> {
    match child.stdin.take() {
        Some(mut stdin) => stdin.write_all(input).context("writing record"),
        None => anyhow::bail!("opening plugin stdin"),
    }
}

/// Wait for output, killing the child after `timeout`.
fn wait_timeout(mut child: Child, timeout: Duration) -> Result<std::process::Output> {
    let start = Instant::now();
    loop {
        match child.try_wait().context("polling plugin")? {
            Some(_) => return child.wait_with_output().context("reading plugin"),
            None if start.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("plugin timed out after {}s", timeout.as_secs());
            }
            None => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// Keep the highest score per (category, value), sorted by score.
pub fn rank(mut suggestions: Vec<Suggestion>) -> Vec<Suggestion> {
    suggestions.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    suggestions.dedup_by(|a, b| a.category == b.category && a.value == b.value);
    suggestions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rank_orders_and_dedupes() {
        let out = rank(vec![
            suggestion("actor", "X", 0.2),
            suggestion("actor", "Y", 0.9),
            suggestion("actor", "Y", 0.9),
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].value, "Y");
    }

    #[test]
    fn suggestion_roundtrips_json() {
        let s = suggestion("actor", "X", 0.5);
        let back: Suggestion = serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        assert_eq!(back, s);
    }

    #[test]
    fn extension_filter_skips() {
        let mut p = ExternalPlugin::new("/bin/true");
        p.extensions = vec!["mp3".to_string()];
        let rec = record_named("film.mkv");
        assert!(p.lookup(&rec, Path::new("/lib")).unwrap().is_empty());
    }

    /// A capture script records its stdin: rels must arrive absolute,
    /// and an empty stored rel must stay empty (not the root itself).
    #[cfg(unix)]
    #[test]
    fn rels_arrive_absolute() {
        use std::os::unix::fs::PermissionsExt;
        let dir = std::env::temp_dir().join(format!("storey-plugincap-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let out = dir.join("got.json");
        let script = dir.join("cap.sh");
        let body = format!("#!/bin/sh\ncat > {out}\nprintf '[]'\n", out = out.display());
        std::fs::write(&script, body).unwrap();
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
        let plugin = ExternalPlugin::new(script.clone());
        let mut rec = record_named("a.mkv");
        rec.original_rel = "films/a.mkv".into();
        rec.stored_rel = ".files/a.mkv".into();
        assert!(plugin.lookup(&rec, Path::new("/lib")).unwrap().is_empty());
        let got: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(got["originalRel"], "/lib/films/a.mkv");
        assert_eq!(got["storedRel"], "/lib/.files/a.mkv");
        rec.stored_rel.clear();
        assert!(plugin.lookup(&rec, Path::new("/lib")).unwrap().is_empty());
        let got: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        assert_eq!(got["storedRel"], "");
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn record_named(display_name: &str) -> Record {
        Record {
            id: "ab".into(),
            display_name: display_name.into(),
            original_rel: display_name.into(),
            stored_rel: String::new(),
            size: 1,
            tags: Default::default(),
        }
    }

    fn suggestion(category: &str, value: &str, score: f32) -> Suggestion {
        Suggestion {
            category: category.into(),
            value: value.into(),
            score,
            source: "test".into(),
        }
    }
}
