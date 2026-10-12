//! Stderr diagnostics. Stdout stays reserved for the `--json`
//! envelope, so everything here goes to stderr: human-readable text
//! by default, one JSON object per line under `--json` for UI
//! progress and warning streams.

use std::sync::Mutex;

use serde::Serialize;

/// Stderr logger. `quiet` suppresses info and progress; warnings
/// and errors always print.
pub struct Logger {
    as_json: bool,
    quiet: bool,
    mu: Mutex<()>,
}

impl Logger {
    /// Build a logger.
    pub fn new(as_json: bool, quiet: bool) -> Self {
        Self {
            as_json,
            quiet,
            mu: Mutex::new(()),
        }
    }

    /// Diagnostic. Suppressed when quiet.
    pub fn info(&self, msg: impl AsRef<str>) {
        if self.quiet {
            return;
        }
        self.emit("info", msg.as_ref(), "", 0, 0, "");
    }

    /// Warning. Always printed: warnings are load-bearing (e.g. a
    /// view link that could not be created).
    pub fn warn(&self, msg: impl AsRef<str>) {
        self.emit("warn", msg.as_ref(), "", 0, 0, "");
    }

    /// Hashing progress: op is `hash`/`verify`, path the
    /// slash-relative file just finished. Suppressed when quiet.
    pub fn progress(&self, op: &str, done: usize, total: usize, path: &str) {
        if self.quiet || total == 0 {
            return;
        }
        self.emit("progress", "", op, done, total, path);
    }

    fn emit(&self, level: &str, msg: &str, op: &str, done: usize, total: usize, path: &str) {
        let _guard = self.mu.lock().unwrap();
        if self.as_json {
            let ev = Event {
                level,
                msg: non_empty(msg),
                op: non_empty(op),
                done: non_zero(done),
                total: non_zero(total),
                path: non_empty(path),
            };
            eprintln!("{}", serde_json::to_string(&ev).unwrap());
            return;
        }
        match level {
            "warn" => eprintln!("storey: warning: {msg}"),
            "info" => eprintln!("storey: {msg}"),
            _ => eprintln!("storey: {op} [{done}/{total}] {path}"),
        }
    }
}

fn non_empty(s: &str) -> Option<&str> {
    if s.is_empty() { None } else { Some(s) }
}

fn non_zero(n: usize) -> Option<usize> {
    if n == 0 { None } else { Some(n) }
}

#[derive(Serialize)]
struct Event<'a> {
    level: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    msg: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    op: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    done: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<&'a str>,
}
