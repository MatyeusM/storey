//! Stderr diagnostics. Stdout stays reserved for the `--json`
//! envelope, so everything here goes to stderr: human-readable text
//! by default, one JSON object per line under `--json` for UI
//! progress and warning streams.

use std::sync::Mutex;

use serde::Serialize;

/// In-process event sink for embedding hosts.
type Tap = Box<dyn Fn(LogEvent) + Send>;

/// Stderr logger. `quiet` suppresses info and progress; warnings
/// and errors always print. A `tap` receives every event in
/// process too (for embedding hosts like the GUI, which never
/// spawn the CLI and so never see stderr).
pub struct Logger {
    as_json: bool,
    quiet: bool,
    mu: Mutex<()>,
    tap: Mutex<Option<Tap>>,
}

/// One log event, shared with in-process taps.
#[derive(Debug, Clone)]
pub struct LogEvent {
    pub level: String,
    pub msg: Option<String>,
    pub op: Option<String>,
    pub done: usize,
    pub total: Option<usize>,
    pub path: String,
}

impl LogEvent {
    /// Hashing/indexing progress ticks.
    pub fn is_progress(&self) -> bool {
        self.level == "progress"
    }
}

impl Logger {
    /// Build a logger.
    pub fn new(as_json: bool, quiet: bool) -> Self {
        Self {
            as_json,
            quiet,
            mu: Mutex::new(()),
            tap: Mutex::new(None),
        }
    }

    /// Receive every event in process (replaces nothing: stderr
    /// output is unchanged).
    pub fn tap(&self, f: impl Fn(LogEvent) + Send + 'static) {
        *self.tap.lock().unwrap() = Some(Box::new(f));
    }

    /// Diagnostic. Suppressed when quiet.
    pub fn info(&self, msg: impl AsRef<str>) {
        if self.quiet {
            return;
        }
        self.emit(LogEvent {
            level: "info".into(),
            msg: Some(msg.as_ref().into()),
            op: None,
            done: 0,
            total: None,
            path: String::new(),
        });
    }

    /// Warning. Always printed: warnings are load-bearing (e.g. a
    /// view link that could not be created).
    pub fn warn(&self, msg: impl AsRef<str>) {
        self.emit(LogEvent {
            level: "warn".into(),
            msg: Some(msg.as_ref().into()),
            op: None,
            done: 0,
            total: None,
            path: String::new(),
        });
    }

    /// Hashing progress: op is `hash`/`verify`, path the
    /// slash-relative file just finished. Suppressed when quiet.
    pub fn progress(&self, op: &str, done: usize, total: usize, path: &str) {
        if self.quiet || total == 0 {
            return;
        }
        self.emit(LogEvent {
            level: "progress".into(),
            msg: None,
            op: Some(op.into()),
            done,
            total: Some(total),
            path: path.into(),
        });
    }

    fn emit(&self, ev: LogEvent) {
        if let Some(tap) = self.tap.lock().unwrap().as_ref() {
            tap(ev.clone());
        }
        let _guard = self.mu.lock().unwrap();
        if self.as_json {
            eprintln!("{}", serde_json::to_string(&EventView::from(&ev)).unwrap());
            return;
        }
        match ev.level.as_str() {
            "warn" => eprintln!("storey: warning: {}", ev.msg.unwrap_or_default()),
            "info" => eprintln!("storey: {}", ev.msg.unwrap_or_default()),
            _ => eprintln!(
                "storey: {} [{}/{}] {}",
                ev.op.unwrap_or_default(),
                ev.done,
                ev.total.unwrap_or(0),
                ev.path
            ),
        }
    }
}

/// JSON view of an event (stderr contract, unchanged).
#[derive(Serialize)]
struct EventView<'a> {
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

impl<'a> EventView<'a> {
    fn from(ev: &'a LogEvent) -> Self {
        let some_unless = |s: &'a str| (!s.is_empty()).then_some(s);
        Self {
            level: &ev.level,
            msg: ev.msg.as_deref(),
            op: ev.op.as_deref(),
            done: (ev.done != 0).then_some(ev.done),
            total: ev.total,
            path: some_unless(&ev.path),
        }
    }
}
