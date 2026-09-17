//! Background long ops: `update`/`verify`, `init`, and `restore` run
//! on a worker thread so the event loop keeps painting a progress bar.
//!
//! [`App::spawn_update`], [`spawn_init`], and [`spawn_restore`] start
//! the worker; [`App::poll_pending`] drains its messages each frame and
//! applies the terminal result (notice or error modal, then refresh).
//! Starting another op while one runs only sets a "busy" notice.

use std::sync::mpsc::{self, Receiver, Sender};

use storey_shared::{
    cli::{CliError, LogEvent},
    model::{RestoreReport, UpdateReport},
};

use crate::app::App;

/// Observable state of a running op, shown at the control bar's right end.
#[derive(Debug, Clone)]
pub struct Progress {
    pub op: String,
    pub done: u64,
    pub total: Option<u64>,
}

impl Progress {
    /// 0.0..=1.0 completion, when the total is known and nonzero.
    pub fn fraction(&self) -> Option<f64> {
        match self.total {
            Some(total) if total > 0 => Some(self.done.min(total) as f64 / total as f64),
            _ => None,
        }
    }

    /// `hash 45% [####------]`, or `hash 12…` when total is unknown.
    pub fn label(&self) -> String {
        match self.fraction() {
            Some(f) => {
                let pct = (f * 100.0).round() as u64;
                let filled = (f * 10.0).round() as usize;
                let bar: String = (0..10).map(|i| hashmark(i < filled)).collect();
                format!("{} {pct}% [{bar}]", self.op)
            }
            None => format!("{} {}…", self.op, self.done),
        }
    }
}

/// One bar cell: filled or empty.
fn hashmark(filled: bool) -> char {
    if filled { '#' } else { '-' }
}

/// Worker → UI messages: progress ticks, then exactly one result.
pub enum Msg {
    Tick(Progress),
    DoneUpdate(Result<UpdateReport, CliError>),
    DoneInit(Result<(), CliError>),
    DoneRestore(Result<RestoreReport, CliError>),
}

/// A running worker and its message channel.
pub struct Pending {
    rx: Receiver<Msg>,
}

/// Forward a hashing progress event to the UI, ignoring the rest.
fn send_tick(tx: &Sender<Msg>, fallback: &str, ev: &LogEvent) {
    if !ev.is_progress() {
        return;
    }
    let op = ev.op.clone().unwrap_or_else(|| fallback.to_string());
    let tick = Progress {
        op,
        done: ev.done.unwrap_or(0),
        total: ev.total,
    };
    let _ = tx.send(Msg::Tick(tick));
}

impl App {
    /// Progress for the control bar: live ticks, or a `working…` stub
    /// while the worker has not sent any yet.
    pub fn bar_progress(&self) -> Option<Progress> {
        self.progress.clone().or_else(|| {
            self.pending.as_ref().map(|_| Progress {
                op: "working".into(),
                done: 0,
                total: None,
            })
        })
    }

    /// True when a worker runs; refuses the new op with a notice.
    fn busy(&mut self) -> bool {
        if self.pending.is_some() {
            self.notice = "busy — wait for the running op".to_string();
            return true;
        }
        false
    }

    /// Run `update` off-thread (`--verify` when asked).
    pub fn spawn_update(&mut self, verify: bool) {
        if self.busy() {
            return;
        }
        self.unack_error();
        let (tx, rx) = mpsc::channel();
        let client = self.client.clone();
        let dir = self.cwd.clone();
        self.pending = Some(Pending { rx });
        std::thread::spawn(move || {
            let result = client.update(&dir, verify, |ev| send_tick(&tx, "hash", ev));
            let _ = tx.send(Msg::DoneUpdate(result));
        });
    }

    /// Run `init` off-thread.
    pub fn spawn_init(&mut self) {
        if self.busy() {
            return;
        }
        self.unack_error();
        let (tx, rx) = mpsc::channel();
        let client = self.client.clone();
        let dir = self.cwd.clone();
        self.pending = Some(Pending { rx });
        std::thread::spawn(move || {
            let result = client.call(&["init"], &dir, |ev| send_tick(&tx, "init", ev));
            let _ = tx.send(Msg::DoneInit(result.map(|_| ())));
        });
    }

    /// Run `restore` off-thread, but only with an empty view.
    pub fn spawn_restore(&mut self) {
        if self.busy() {
            return;
        }
        if !self.view.is_empty() {
            self.notice = "restore needs an empty view".to_string();
            return;
        }
        self.unack_error();
        let (tx, rx) = mpsc::channel();
        let client = self.client.clone();
        let dir = self.cwd.clone();
        self.pending = Some(Pending { rx });
        std::thread::spawn(move || {
            let result = client.restore(&dir, |ev| send_tick(&tx, "restore", ev));
            let _ = tx.send(Msg::DoneRestore(result));
        });
    }

    /// Drain worker messages; apply a terminal result once per frame.
    pub fn poll_pending(&mut self) {
        let msgs: Vec<Msg> = match self.pending.as_ref() {
            Some(pending) => pending.rx.try_iter().collect(),
            None => return,
        };
        for msg in msgs {
            match msg {
                Msg::Tick(p) => self.progress = Some(p),
                Msg::DoneUpdate(r) => return self.finish_update(r),
                Msg::DoneInit(r) => return self.finish_init(r),
                Msg::DoneRestore(r) => return self.finish_restore(r),
            }
        }
    }

    /// Apply an `update` outcome, then reload the repo state.
    fn finish_update(&mut self, result: Result<UpdateReport, CliError>) {
        self.pending = None;
        self.progress = None;
        match result {
            Ok(report) => self.notice = report.summary(),
            Err(e) => match update_report(&e) {
                Some(report) => self.notice = report.summary(),
                None => self.fail("update", &e),
            },
        }
        self.refresh();
        if self.is_repo {
            self.load_records();
        }
    }

    /// Apply an `init` outcome, then refresh into the new repo.
    fn finish_init(&mut self, result: Result<(), CliError>) {
        self.pending = None;
        self.progress = None;
        match result {
            Ok(()) => self.notice = "initialized".to_string(),
            Err(e) => self.fail("init", &e),
        }
        self.refresh();
    }

    /// Apply a `restore` outcome, then reload the repo state.
    fn finish_restore(&mut self, result: Result<RestoreReport, CliError>) {
        self.pending = None;
        self.progress = None;
        match result {
            Ok(report) if report.blocked() => {
                self.notice = format!("restore blocked: {}", report.blockers());
            }
            Ok(report) => self.notice = report.summary(),
            Err(e) => match restore_report(&e) {
                Some(report) if report.blocked() => {
                    self.notice = format!("restore blocked: {}", report.blockers());
                }
                Some(report) => self.notice = report.summary(),
                None => self.fail("restore", &e),
            },
        }
        self.refresh();
        if self.is_repo {
            self.load_records();
        }
    }
}

/// Decode a report from a report-only exit-1 envelope.
fn update_report(e: &CliError) -> Option<UpdateReport> {
    decode_report(e)
}

/// Decode a restore report from a blocked/refusing exit-1 envelope.
fn restore_report(e: &CliError) -> Option<RestoreReport> {
    decode_report(e)
}

/// Shared body: the `data` payload of a failing-but-reporting call.
fn decode_report<T: serde::de::DeserializeOwned>(e: &CliError) -> Option<T> {
    let envelope = e.envelope.as_ref()?;
    serde_json::from_value(envelope.data.clone()).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-progress-{}", std::process::id()));
        let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
        app.close_error();
        app
    }

    #[test]
    fn label_known_and_unknown_totals() {
        let known = Progress {
            op: "hash".into(),
            done: 1,
            total: Some(2),
        };
        assert_eq!(known.fraction(), Some(0.5));
        assert_eq!(known.label(), "hash 50% [#####-----]");
        let unknown = Progress {
            op: "hash".into(),
            done: 7,
            total: None,
        };
        assert_eq!(unknown.fraction(), None);
        assert_eq!(unknown.label(), "hash 7…");
    }

    #[test]
    fn ticks_update_without_finishing() {
        let (tx, rx) = mpsc::channel();
        let mut app = test_app();
        app.pending = Some(Pending { rx });
        tx.send(Msg::Tick(Progress {
            op: "hash".into(),
            done: 3,
            total: Some(4),
        }))
        .unwrap();
        app.poll_pending();
        assert!(app.pending.is_some());
        assert_eq!(app.progress.as_ref().unwrap().done, 3);
        assert!(app.error.is_none());
    }

    #[test]
    fn second_spawn_is_refused_busy() {
        let (_tx, rx) = mpsc::channel();
        let mut app = test_app();
        app.pending = Some(Pending { rx });
        app.spawn_update(false);
        assert_eq!(app.notice, "busy — wait for the running op");
    }
}
