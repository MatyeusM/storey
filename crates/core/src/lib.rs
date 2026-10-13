//! storey-core: library logic behind the `storey` CLI.
//!
//! The CLI contract (args, JSON envelopes, exit codes) is pinned in
//! `docs/CLI-CONTRACT.md`. This crate owns the mechanics; `apps/cli`
//! owns argument parsing and process wiring.

pub mod adopt;

/// Outcome of a flow: report payload plus whether problems
/// (report-only) or an abort occurred.
pub enum FlowDone<T> {
    /// Finished; problems (if any) ride in the report.
    Done(T),
    /// Abort flag set: partial state already applied.
    Aborted,
}
pub mod flows;
pub mod hash;
pub mod index;
pub mod intake;
pub mod library;
pub mod lock;
pub mod log;
pub mod materialize;
pub mod materialize_fs;
pub mod ops;
pub mod remove;
pub mod rename;
pub mod restore;
pub mod tags;
pub mod views;
