//! storey-core: library logic behind the `storey` CLI.
//!
//! The CLI contract (args, JSON envelopes, exit codes) is pinned in
//! `docs/CLI-CONTRACT.md`. This crate owns the mechanics; `apps/cli`
//! owns argument parsing and process wiring.

pub mod adopt;
pub mod flows;
pub mod hash;
pub mod index;
pub mod intake;
pub mod library;
pub mod lock;
pub mod log;
pub mod ops;
pub mod views;
