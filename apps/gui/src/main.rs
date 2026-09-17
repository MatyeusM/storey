//! storey-gui binary entrypoint: boots Qt and loads the QML shell.
//!
//! The UI itself is QML (`qml/`); cxx-qt is the thin wrapper that
//! starts the application and exposes Rust objects to it. All library
//! logic lives in the `storey` CLI, driven through `storey-shared`
//! (see `docs/CLI-CONTRACT.md`).

use clap::Parser;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

mod library;
mod nav;
mod places;
mod plugins;
mod state;
mod store;
mod support;
mod tagging;

/// Thin graphical client over the storey CLI.
#[derive(Parser, Debug)]
#[command(name = "storey-gui", version, about = "GUI for storey")]
struct Args {
    /// Path to the storey binary (else ui.toml, sibling, or PATH).
    #[arg(long)]
    storey_bin: Option<String>,
    /// Directory to open on start (defaults to cwd).
    #[arg(default_value = ".")]
    path: std::path::PathBuf,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let start = if args.path.is_absolute() {
        args.path.clone()
    } else {
        std::env::current_dir()?.join(args.path.clone())
    };
    // The QObject is constructed from QML, so hand over the resolved
    // binary and start dir through the environment for `Default`.
    // SAFETY: single-threaded startup before Qt spawns threads.
    unsafe {
        std::env::set_var("STOREY_BIN", resolve_bin(args.storey_bin.as_deref()));
        std::env::set_var("STOREY_START_DIR", start.display().to_string());
    }

    let mut app = QGuiApplication::new();
    // Note: these setters take Pin<&mut> by value, so each needs a
    // fresh borrow.
    if let Some(app) = app.as_mut() {
        app.set_application_name(&QString::from("storey-gui"));
    }
    if let Some(app) = app.as_mut() {
        app.set_application_version(&QString::from(storey_shared::VERSION));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(engine) = engine.as_mut() {
        // Fail loudly on scene errors: without this a bad QML
        // module spins an empty event loop with no window.
        engine
            .on_object_created(|_, object, _| {
                if object.is_null() {
                    eprintln!("storey-gui: failed to load QML scene");
                    std::process::exit(1);
                }
            })
            .release();
    }
    if let Some(engine) = engine.as_mut() {
        engine.load(&QUrl::from("qrc:/qt/qml/com/storey/gui/qml/Main.qml"));
    }
    std::process::exit(app.pin_mut().exec());
}

/// Resolve which `storey` binary to drive: flag, `ui.toml`, sibling.
/// Relative paths are anchored at the process cwd: CLI calls run
/// with `current_dir` set to the open folder, so a bare relative
/// path would break on navigation (see `storey-shared::binpath`).
fn resolve_bin(explicit: Option<&str>) -> String {
    let bin = if let Some(bin) = explicit.filter(|s| !s.is_empty()) {
        bin.to_string()
    } else if let Some(bin) = configured_custom() {
        bin
    } else {
        sibling().unwrap_or_else(|| configured().unwrap_or_else(|| "storey".into()))
    };
    absolutize(&bin)
}

/// Anchor a configured binary at the startup directory, if relative.
fn absolutize(bin: &str) -> String {
    let path = std::path::PathBuf::from(bin);
    match std::env::current_dir() {
        Ok(cwd) => storey_shared::binpath::absolutize(&path, &cwd)
            .display()
            .to_string(),
        Err(_) => bin.to_string(),
    }
}

/// `storey_bin` from `ui.toml`, when the file exists and parses.
fn configured() -> Option<String> {
    let path = storey_shared::settings::config_path()?;
    let text = std::fs::read_to_string(&path).ok()?;
    let settings = storey_shared::settings::parse(&text).ok()?;
    let bin = settings.storey_bin.trim().to_string();
    if bin.is_empty() { None } else { Some(bin) }
}

/// `ui.toml` value when it differs from the PATH default.
fn configured_custom() -> Option<String> {
    let bin = configured()?;
    if bin == "storey" { None } else { Some(bin) }
}

/// A `storey` shipped next to this executable (release installs).
fn sibling() -> Option<String> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    let cand = dir.join("storey");
    if cand.is_file() {
        Some(cand.display().to_string())
    } else {
        None
    }
}
