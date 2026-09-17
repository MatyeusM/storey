//! Drive the `storey` CLI as a subprocess.
//!
//! Every invocation runs `bin --json <args...>` in a caller-chosen
//! directory with stdout captured: exactly one [`Envelope`] object,
//! even on failures that still emit `data` (for example `view`
//! reporting `untracked` before exiting 1). Stderr carries human text
//! or, under `--json`, one [`LogEvent`] per line.
//!
//! [`Client`] is the typed entry point; [`commands`] adds one method
//! per contract command on top of [`Client::call`].

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

/// The `--json` stdout envelope: `{"schema":1,"command":..,"data":..}`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Envelope {
    pub schema: u32,
    pub command: String,
    pub data: serde_json::Value,
}

/// One stderr JSONL event (see `docs/CLI-CONTRACT.md`, Logging).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEvent {
    pub level: String,
    #[serde(default)]
    pub msg: Option<String>,
    #[serde(default)]
    pub op: Option<String>,
    #[serde(default)]
    pub done: Option<u64>,
    #[serde(default)]
    pub total: Option<u64>,
    #[serde(default)]
    pub path: Option<String>,
}

impl LogEvent {
    /// True for hashing progress events (`op` is `hash` or `verify`).
    pub fn is_progress(&self) -> bool {
        self.level == "progress"
    }

    /// 0.0..=1.0 completion, or None when the total is unknown.
    pub fn fraction(&self) -> Option<f64> {
        match (self.done, self.total) {
            (Some(done), Some(total)) if total > 0 => Some(done.min(total) as f64 / total as f64),
            _ => None,
        }
    }
}

/// A finished invocation: the parsed envelope plus every log event.
#[derive(Debug)]
pub struct Output {
    pub envelope: Envelope,
    pub logs: Vec<LogEvent>,
}

/// Failure of a CLI invocation: exit code, stderr tail, and the
/// envelope when the CLI still emitted `data` before failing.
#[derive(Debug)]
pub struct CliError {
    pub code: Option<i32>,
    pub message: String,
    pub envelope: Option<Envelope>,
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.code {
            Some(code) => write!(f, "storey exited with {code}: {}", self.message),
            None => write!(f, "storey failed: {}", self.message),
        }
    }
}

impl std::error::Error for CliError {}

/// Typed driver for the `storey` binary (`--storey-bin` override).
#[derive(Debug, Clone)]
pub struct Client {
    bin: PathBuf,
}

impl Client {
    /// Point at a `storey` binary (plain name uses `PATH`).
    pub fn new(bin: impl Into<PathBuf>) -> Self {
        Self { bin: bin.into() }
    }

    /// Binary under test or distro override.
    pub fn bin(&self) -> &Path {
        &self.bin
    }

    /// Run `bin --json <args...>` in `dir`, collecting log events.
    /// The envelope is parsed even on non-zero exits so callers keep
    /// the `data` payload (exit 2 means usage / not-a-library).
    /// Stderr streams live: progress ticks reach `on_log` while the
    /// child runs, not batched at exit.
    pub fn call(
        &self,
        args: &[&str],
        dir: &Path,
        mut on_log: impl FnMut(&LogEvent),
    ) -> Result<Output, CliError> {
        let mut child = Command::new(&self.bin);
        child.arg("--json").args(args);
        child.stdout(Stdio::piped()).stderr(Stdio::piped());
        child.current_dir(dir);
        let mut child = child.spawn().map_err(|e| CliError {
            code: None,
            message: format!("spawning storey: {e}"),
            envelope: None,
        })?;
        let mut stderr = child.stderr.take();
        let mut stdout = child.stdout.take();
        let (tx, rx) = std::sync::mpsc::channel::<String>();
        let mut out_bytes = Vec::new();
        let mut logs = Vec::new();
        let mut raw_lines = Vec::new();
        std::thread::scope(|s| {
            // NOTE: `move` is load-bearing — without it the closure
            // borrows `tx`, the owner outlives the drain loop, the
            // receiver never disconnects, and the parent deadlocks.
            s.spawn(move || {
                if let Some(pipe) = stderr.as_mut() {
                    let mut reader = std::io::BufReader::new(pipe);
                    let mut chunk = Vec::new();
                    loop {
                        chunk.clear();
                        match std::io::BufRead::read_until(&mut reader, b'\n', &mut chunk) {
                            Ok(0) => break,
                            Ok(_) => {
                                let _ = tx.send(String::from_utf8_lossy(&chunk).into_owned());
                            }
                            Err(_) => break,
                        }
                    }
                }
            });
            s.spawn(|| {
                if let Some(pipe) = stdout.as_mut() {
                    let _ = std::io::Read::read_to_end(pipe, &mut out_bytes);
                }
            });
            // Parent drains live: ticks reach on_log while the child
            // runs. Raw lines are kept so human-text error tails
            // survive alongside parsed events.
            for line in rx {
                raw_lines.push(line.clone());
                if let Some(event) = parse_log_line(&line) {
                    on_log(&event);
                    logs.push(event);
                }
            }
        });
        let status = child.wait().map_err(|e| CliError {
            code: None,
            message: format!("waiting for storey: {e}"),
            envelope: None,
        })?;
        let stdout = String::from_utf8_lossy(&out_bytes);
        let envelope = stdout
            .lines()
            .find(|l| !l.trim().is_empty())
            .and_then(|l| parse_envelope(l).ok());
        match (status.code(), envelope) {
            (Some(0), Some(envelope)) => Ok(Output { envelope, logs }),
            (code, envelope) => {
                let tail = raw_lines.last().cloned().unwrap_or_default();
                Err(CliError {
                    code,
                    message: tail.trim().to_string(),
                    envelope,
                })
            }
        }
    }
}

/// Build `bin` as a child command (no `--json` yet — see [`Client`]).
/// Both UIs start from this so the spawn setup stays identical.
pub fn command(bin: &str) -> Command {
    Command::new(bin)
}

/// Parse one stdout line into an [`Envelope`].
pub fn parse_envelope(line: &str) -> anyhow::Result<Envelope> {
    serde_json::from_str(line.trim()).map_err(|e| anyhow::anyhow!("parsing storey envelope: {e}"))
}

/// Parse one stderr line into a [`LogEvent`]. Human-readable lines
/// (no `--json`) yield `None` instead of an error.
pub fn parse_log_line(line: &str) -> Option<LogEvent> {
    serde_json::from_str(line.trim()).ok()
}

/// Run `bin --json <args...>` in the current directory.
/// Prefer [`Client::call`] with an explicit directory.
pub fn run(bin: &str, args: &[&str], on_log: impl FnMut(&LogEvent)) -> Result<Output, CliError> {
    let dir = std::env::current_dir().map_err(|e| CliError {
        code: None,
        message: format!("current dir: {e}"),
        envelope: None,
    })?;
    Client::new(bin).call(args, &dir, on_log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn envelope_parses() {
        let env = parse_envelope(r#"{"schema":1,"command":"list","data":[]}"#).unwrap();
        assert_eq!(env.command, "list");
    }

    #[test]
    fn log_line_json_or_none() {
        let event =
            parse_log_line(r#"{"level":"progress","op":"hash","done":1,"total":2}"#).unwrap();
        assert!(event.is_progress());
        assert_eq!(event.fraction(), Some(0.5));
        assert!(parse_log_line("storey: hashing [1/2] a.txt").is_none());
    }

    #[test]
    fn client_points_at_bin() {
        assert_eq!(Client::new("storey").bin(), Path::new("storey"));
    }

    #[cfg(unix)]
    fn probe_script(body: &str) -> PathBuf {
        use std::io::Write;
        let path = std::env::temp_dir().join(format!(
            "storey-probe-{}-{}.sh",
            std::process::id(),
            body.len()
        ));
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(f, "#!/bin/sh").unwrap();
        writeln!(f, "{body}").unwrap();
        drop(f);
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    /// Ticks arrive while the child runs (span covers the sleeps),
    /// in order, and the envelope still parses.
    #[cfg(unix)]
    #[test]
    fn progress_streams_while_child_runs() {
        use std::time::Instant;
        let path = probe_script(
            r#"echo '{"level":"progress","op":"hash","done":1,"total":3}' >&2
               sleep 0.4
               echo '{"level":"progress","op":"hash","done":2,"total":3}' >&2
               sleep 0.4
               echo '{"schema":1,"command":"probe","data":[]}'"#,
        );
        let client = Client::new(&path);
        let mut seen = Vec::new();
        let out = client
            .call(&[], Path::new("/tmp"), |ev| {
                seen.push((ev.done, Instant::now()));
            })
            .unwrap();
        assert_eq!(out.envelope.command, "probe");
        assert_eq!(seen.len(), 2);
        assert_eq!(seen[0].0, Some(1));
        assert_eq!(seen[1].0, Some(2));
        assert!(seen[1].1.duration_since(seen[0].1) >= std::time::Duration::from_millis(150));
        let _ = std::fs::remove_file(&path);
    }

    /// Human stderr tails survive for error messages.
    #[cfg(unix)]
    #[test]
    fn human_error_tail_survives() {
        let path = probe_script("echo 'storey: probe exploded' >&2\nexit 3");
        let client = Client::new(&path);
        let err = client.call(&[], Path::new("/tmp"), |_| {}).unwrap_err();
        assert_eq!(err.code, Some(3));
        assert_eq!(err.message, "storey: probe exploded");
        let _ = std::fs::remove_file(&path);
    }
}
