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
        self.emit(Event {
            level: "info",
            msg: Some(msg.as_ref()),
            op: None,
            done: None,
            total: None,
            path: None,
        });
    }

    /// Warning. Always printed: warnings are load-bearing (e.g. a
    /// view link that could not be created).
    pub fn warn(&self, msg: impl AsRef<str>) {
        self.emit(Event {
            level: "warn",
            msg: Some(msg.as_ref()),
            op: None,
            done: None,
            total: None,
            path: None,
        });
    }

    /// Hashing progress: op is `hash`/`verify`, path the
    /// slash-relative file just finished. Suppressed when quiet.
    pub fn progress(&self, op: &str, done: usize, total: usize, path: &str) {
        if self.quiet || total == 0 {
            return;
        }
        self.emit(Event {
            level: "progress",
            msg: None,
            op: Some(op),
            done: Some(done),
            total: Some(total),
            path: Some(path),
        });
    }

    fn emit(&self, ev: Event) {
        let _guard = self.mu.lock().unwrap();
        if self.as_json {
            eprintln!("{}", serde_json::to_string(&ev).unwrap());
            return;
        }
        match ev.level {
            "warn" => eprintln!("storey: warning: {}", ev.msg.unwrap_or("")),
            "info" => eprintln!("storey: {}", ev.msg.unwrap_or("")),
            _ => eprintln!(
                "storey: {} [{}/{}] {}",
                ev.op.unwrap_or(""),
                ev.done.unwrap_or(0),
                ev.total.unwrap_or(0),
                ev.path.unwrap_or("")
            ),
        }
    }
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
