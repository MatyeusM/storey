//! ui: Qt-free interface logic over `core`.
//!
//! `apps/gui` is the thin Qt/QML shell: it mirrors its properties
//! into [`State`], calls one action, and mirrors back. Everything
//! else — syncing, filtering, tagging, plugins, theming inputs —
//! lives here and is unit-tested without Qt.

pub mod explore;
pub mod library;
pub mod manifest;
pub mod nav;
pub mod places;
pub mod plugins;
pub mod settings;
pub mod state;
pub mod support;
pub mod tagging;
pub mod tags;
pub mod theme;

pub use state::State;
