//! `storey` thin CLI: argument parsing plus response printing.
//!
//! All library mechanics live in `storey-core` (`crates/storey-core`);
//! this binary maps argv to ops calls and prints the `--json`
//! envelope or human tables. Exit codes per `docs/CLI-CONTRACT.md`:
//! 0 ok, 1 failure (problems report), 2 usage, 130 interrupted.

use clap::{Parser, Subcommand};

mod cmd_intake;
mod cmd_mv;
mod cmd_read;
mod cmd_restore;
mod cmd_rm;
mod cmd_tag;
mod cmd_view;
mod cmd_view_report;

/// Process exit codes.
const EXIT_OK: i32 = 0;
const EXIT_FAILURE: i32 = 1;
const EXIT_USAGE: i32 = 2;

/// Thin CLI over the storey library.
#[derive(Parser)]
#[command(
    name = "storey",
    version,
    about = "CAS with file-manager-friendly views",
    disable_help_flag = true,
    disable_version_flag = true
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create .index.json here, adopt nested libraries, hash in place.
    Init {
        /// Raw arguments (only --dry-run).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Fold nested libraries, hash new files, optionally re-verify.
    Update {
        /// Raw arguments (--verify, --dry-run).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Read-only overview: root, counts, pending files, view order.
    Status {
        /// Raw arguments (none take).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Tag reads and writes (get, set, add, remove).
    Tag {
        /// Subcommand plus its raw arguments.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Ingest into .files/, wipe, rebuild links, record key order.
    View {
        /// Raw arguments (--tags required, plus --force/--dry-run).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Delete links, file, and record. Permanent.
    #[command(alias = "rm")]
    Remove {
        /// Raw arguments (one target plus --dry-run).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Rename display name, file, and view links.
    Mv {
        /// Raw arguments (target, new name, plus --dry-run).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// Remove view artifacts, move objects back.
    Restore {
        /// Raw arguments (--force, --dry-run).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    /// All records, optionally one category=value filter or --untagged.
    List {
        /// Raw arguments (--untagged plus at most one filter).
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
}

fn main() {
    std::process::exit(run());
}

fn run() -> i32 {
    // Float --json/--quiet wherever they appear (mirrors the
    // reference CLI): no command accepts these positionally.
    let mut as_json = false;
    let mut quiet = false;
    let mut rest = vec!["storey".to_string()];
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "--json" => as_json = true,
            "--quiet" | "-q" => quiet = true,
            _ => rest.push(a),
        }
    }
    if let Some(code) = pre_dispatch(&rest) {
        return code;
    }
    let cli = match Cli::try_parse_from(&rest) {
        Ok(cli) => cli,
        Err(e)
            if e.kind() == clap::error::ErrorKind::InvalidSubcommand
                || e.kind() == clap::error::ErrorKind::UnknownArgument =>
        {
            // Unknown command (or a bare `--help` in command position,
            // since help is manual here): Go-style notice plus usage.
            return unknown_command(rest.get(1).map(|s| s.as_str()).unwrap_or(""));
        }
        Err(e) => e.exit(),
    };
    let invoked = rest.get(1).cloned().unwrap_or_default();
    let logger = storey_core::log::Logger::new(as_json, quiet);
    match &cli.command {
        Commands::Init { args } => cmd_intake::cmd_init(args, as_json, &logger),
        Commands::Update { args } => cmd_intake::cmd_update(args, as_json, &logger),
        Commands::Status { args } => cmd_read::cmd_status(args, as_json, &logger),
        Commands::Tag { args } => cmd_tag::cmd_tag(args, as_json),
        Commands::View { args } => cmd_view::cmd_view(args, as_json, &logger),
        Commands::Remove { args } => cmd_rm::cmd_rm(args, &invoked, as_json, &logger),
        Commands::Mv { args } => cmd_mv::cmd_mv(args, as_json, &logger),
        Commands::Restore { args } => cmd_restore::cmd_restore(args, as_json, &logger),
        Commands::List { args } => cmd_read::cmd_list(args, as_json),
    }
}

/// Leading `--help`/`--version`/empty handling plus the usage text,
/// mirroring the reference CLI.
fn pre_dispatch(rest: &[String]) -> Option<i32> {
    match rest.get(1).map(|s| s.as_str()) {
        None => {
            usage();
            Some(EXIT_USAGE)
        }
        Some("--help") | Some("-h") => {
            usage();
            Some(EXIT_OK)
        }
        Some("--version") => {
            println!("storey {}", env!("CARGO_PKG_VERSION"));
            Some(EXIT_OK)
        }
        _ => None,
    }
}

/// Unknown command notice plus usage (exit 2).
fn unknown_command(cmd: &str) -> i32 {
    if !cmd.is_empty() {
        eprintln!("storey: unknown command {cmd:?}");
    }
    usage();
    EXIT_USAGE
}

/// Program usage on stdout.
fn usage() {
    println!("storey {}", env!("CARGO_PKG_VERSION"));
    println!("usage: storey [--json] [--quiet] <command> [args]");
    println!("commands: init update view list tag remove|rm mv restore status");
    println!("logs and hashing progress go to stderr (JSONL under --json);");
    println!("see docs/CLI-CONTRACT.md");
}

/// Print the stdout envelope (schema 1) under `--json`.
/// Field order matches the reference CLI byte-for-byte.
fn emit(command: &str, as_json: bool, data: &impl serde::Serialize) {
    if as_json {
        let env = Envelope {
            schema: 1,
            command,
            data,
        };
        println!("{}", serde_json::to_string(&env).unwrap());
    }
}

#[derive(serde::Serialize)]
struct Envelope<'a, T: serde::Serialize> {
    schema: u32,
    command: &'a str,
    data: &'a T,
}

/// Runtime failure (exit 1).
fn fail(msg: &str) -> i32 {
    eprintln!("storey: {msg}");
    EXIT_FAILURE
}

/// Usage error (exit 2).
fn usage_err(msg: &str) -> i32 {
    eprintln!("storey: {msg}");
    EXIT_USAGE
}

/// Resolve a target: emit the failure payload and print, or return
/// the record position.
fn resolve_target(
    root: &std::path::Path,
    ix: &storey_core::index::Index,
    arg: &str,
    command: &str,
    as_json: bool,
) -> Result<usize, ()> {
    match storey_core::tags::resolve_target(root, ix, arg) {
        Ok(pos) => Ok(pos),
        Err(e) => {
            emit(command, as_json, &e);
            match e.candidates {
                Some(c) => fail(&format!("ambiguous id prefix {arg:?}: {}", c.join(", "))),
                None => fail(&e.error),
            };
            Err(())
        }
    }
}

/// Trap Ctrl-C (plus SIGTERM on unix) into a flag the flows poll.
fn trap_abort() -> std::sync::Arc<std::sync::atomic::AtomicBool> {
    let abort = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    #[cfg(unix)]
    for sig in [signal_hook::consts::SIGINT, signal_hook::consts::SIGTERM] {
        let _ = signal_hook::flag::register(sig, std::sync::Arc::clone(&abort));
    }
    #[cfg(not(unix))]
    let _ = signal_hook::flag::register(signal_hook::consts::SIGINT, std::sync::Arc::clone(&abort));
    abort
}

/// Save a partial index after an abort (exit 130); the lock guard
/// still drops on the way out.
fn emergency_save(root: &std::path::Path, ix: &storey_core::index::Index) -> i32 {
    match ix.save(root) {
        Ok(()) => eprintln!("storey: interrupted, index saved"),
        Err(e) => eprintln!("storey: interrupted: {e} (index may be stale)"),
    }
    130
}

/// Opened library: root, held lock, loaded index.
struct Opened {
    root: std::path::PathBuf,
    _lock: storey_core::lock::Guard,
    ix: storey_core::index::Index,
}

/// Resolve cwd, claim the lock, load the index. Failures print
/// their own message and yield the exit code.
fn open_locked() -> Result<Opened, i32> {
    let cwd = std::env::current_dir().map_err(|e| fail(&format!("{e}")))?;
    let root = storey_core::library::find_root(&cwd).map_err(|e| usage_err(&format!("{e}")))?;
    let lock = storey_core::lock::acquire(&root).map_err(|msg| fail(&msg))?;
    let ix = storey_core::index::Index::load(&root).map_err(|e| fail(&format!("{e}")))?;
    Ok(Opened {
        root,
        _lock: lock,
        ix,
    })
}
