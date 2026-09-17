//! storey-tui: ratatui terminal interface to the `storey` CLI.
//!
//! Hello-world stage: boots the terminal, draws one screen, quits on
//! `q`/Esc. All CAS/index/view logic lives in `apps/cli` and is driven
//! as a subprocess through `storey-shared` (see `docs/CLI-CONTRACT.md`).

use clap::Parser;

mod app;
mod bin;
mod browse;
mod controls;
mod detail;
mod detail_view;
mod dialog;
mod error;
mod files;
mod filter;
mod fs;
mod keys;
mod manage;
mod plugins_view;
mod progress;
mod records;
mod run;
mod tagbar;
mod topbar;
mod ui;

/// Thin terminal client over the storey CLI.
#[derive(Parser, Debug)]
#[command(name = "storey-tui", version, about = "TUI for storey")]
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
    let resolved = bin::resolve(args.storey_bin.as_deref());
    run::run(app::App::new(resolved, start))
}
