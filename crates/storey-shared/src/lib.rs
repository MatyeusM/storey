//! storey-shared: common UI layer for `storey-gui`.
//!
//! The GUI drives the `storey` CLI as a subprocess (see
//! `docs/CLI-CONTRACT.md`) and never touch the store or index directly.
//! This crate holds what they share:
//!
//! - [`binpath`]: anchor relative `storey` binary paths at the
//!   startup directory (CLI calls run with `current_dir` set).
//! - [`cli`]: typed [`cli::Client`] driver, stdout envelope and stderr
//!   JSONL log parsing (progress, warnings), plus [`cli::CliError`].
//! - [`commands`]: one [`cli::Client`] method per contract command,
//!   parsing `data` into [`model`] types for direct UI use.
//! - [`model`]: record, status, and view-report types with display
//!   helpers for tables and summaries.
//! - [`tags`]: plugin-only tag suggesters ([`tags::ExternalPlugin`])
//!   plus [`tags::Suggestion`] and [`tags::rank`].
//! - [`settings`]: `storey/ui.toml` in the OS config dir (plugin list,
//!   `storey_bin` override, `[gui]` design tokens) with load/save helpers.
//! - [`theme`]: GUI design tokens ([`theme::GuiTheme`] defaults plus
//!   [`theme::GuiOverrides`] from `[gui]`).
//! - [`explore`]: explorer directory listing (dirs first, dotfiles
//!   and `.files/` hidden) used by the GUI browser.
//! - [`filter`]: live record narrowing by name, tags, or lack of tags.
//! - [`manifest`]: plugin `manifest.json` for directory plugins
//!   (entrypoint, supported extensions, protocol check).

pub mod binpath;
pub mod cli;
pub mod commands;
pub mod explore;
pub mod filter;
pub mod manifest;
pub mod model;
pub mod settings;
pub mod tags;
pub mod theme;

pub use cli::Client;

/// Crate version, kept in sync with the repo-root `VERSION` file.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
