//! storey-gui binary entrypoint: boots Qt and loads the QML shell.
//!
//! The UI itself is QML (`qml/`); cxx-qt is the thin wrapper that
//! starts the application and exposes objects to it. All logic
//! lives in linked crates (`core` mechanics, `ui` state); the GUI
//! never spawns a subprocess (see `docs/CLI-CONTRACT.md`).

use clap::Parser;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

mod library;
mod nav;
mod plugins;
mod store;
mod support;
mod tagging;

/// Thin graphical interface over linked crates.
#[derive(Parser, Debug)]
#[command(name = "storey-gui", version, about = "GUI for storey")]
struct Args {
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
    // The QObject is constructed from QML, so hand over the start
    // dir through the environment for `Default`.
    // SAFETY: single-threaded startup before Qt spawns threads.
    unsafe {
        std::env::set_var("STOREY_START_DIR", start.display().to_string());
    }

    let mut app = QGuiApplication::new();
    // Note: these setters take Pin<&mut> by value, so each needs a
    // fresh borrow.
    if let Some(app) = app.as_mut() {
        app.set_application_name(&QString::from("storey-gui"));
    }
    if let Some(app) = app.as_mut() {
        app.set_application_version(&QString::from(env!("CARGO_PKG_VERSION")));
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
