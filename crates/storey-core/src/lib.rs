//! storey-core: library logic behind the `storey` CLI.
//!
//! The CLI contract (args, JSON envelopes, exit codes) is pinned in
//! `docs/CLI-CONTRACT.md`. This crate owns the mechanics; `apps/cli`
//! owns argument parsing and process wiring.

pub mod hash;
pub mod index;
pub mod library;
pub mod log;
pub mod ops;
pub mod views;
