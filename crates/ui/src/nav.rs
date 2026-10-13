//! Navigation actions: refresh, up/open/enter, explorer,
//! status clearing. Pure state transitions; threads and Qt stay in
//! `apps/gui`.
//!
//! Entering a directory inside a library is blocked on purpose:
//! the view is regenerable output, so drilling in would only
//! confuse — the status bar says to use the file explorer instead.

use std::path::{Path, PathBuf};

use crate::{places, state::State, support};

/// Init flow: refuse inside libraries, lock, fresh index, exec.
/// Maps aborts and errors to status details for `finish_init`.
pub fn run_init_flow(cwd: &Path, logger: &storey_core::log::Logger) -> Result<(), String> {
    use std::sync::atomic::AtomicBool;
    if storey_core::library::find_root(cwd).is_ok() {
        return Err("already inside a library".into());
    }
    let _lock = storey_core::lock::acquire(cwd)?;
    let abort = AtomicBool::new(false);
    let mut ix = storey_core::index::Index::new();
    match storey_core::flows::exec_init(cwd, &mut ix, logger, &abort) {
        Ok(storey_core::flows::FlowDone::Done(_)) => Ok(()),
        Ok(storey_core::flows::FlowDone::Aborted) => Err(support::emergency_save(cwd, &ix)),
        Err(e) => Err(support::short_detail(&e.to_string())),
    }
}

/// Update flow: lock, load, exec. Untrusted here means no verify.
pub fn run_update_flow(
    cwd: &Path,
    logger: &storey_core::log::Logger,
) -> Result<storey_core::flows::UpdateReport, String> {
    use std::sync::atomic::AtomicBool;
    let root =
        storey_core::library::find_root(cwd).map_err(|e| support::short_detail(&e.to_string()))?;
    let _lock = storey_core::lock::acquire(&root)?;
    let mut ix = storey_core::index::Index::load(&root)
        .map_err(|e| support::short_detail(&e.to_string()))?;
    let abort = AtomicBool::new(false);
    match storey_core::flows::exec_update(&root, &mut ix, false, logger, &abort) {
        Ok(storey_core::flows::FlowDone::Done(rep)) => Ok(rep),
        Ok(storey_core::flows::FlowDone::Aborted) => Err(support::emergency_save(&root, &ix)),
        Err(e) => Err(support::short_detail(&e.to_string())),
    }
}

/// Re-read everything.
pub fn refresh(state: &mut State) {
    crate::state::sync_all(state);
}

/// Move to the parent directory, then always refresh.
pub fn go_up(state: &mut State) {
    let cwd = PathBuf::from(state.cwd.clone());
    if let Some(parent) = cwd.parent() {
        move_cwd(state, &parent.display().to_string());
    }
    refresh(state);
}

/// Enter directories; files are a no-op for the shell.
pub fn open_path(state: &mut State, path: &str) {
    let target = PathBuf::from(path);
    if target.is_dir() {
        move_cwd(state, &target.display().to_string());
        clear_status(state);
        refresh(state);
    }
}

/// Navigate only outside repos; inside a library the shell opens
/// the OS file explorer instead.
pub fn enter_dir(state: &mut State, path: &str) {
    if let Some(msg) = blocked_message(state.is_repo) {
        state.status_text = msg;
        return;
    }
    let target = PathBuf::from(path);
    if target.is_dir() {
        move_cwd(state, &target.display().to_string());
        refresh(state);
    }
}

/// Clear plain notices; error lines persist.
pub fn clear_status(state: &mut State) {
    if state.status_text.starts_with("storey error:")
        || state.status_text.starts_with("storey error (")
    {
        return;
    }
    state.status_text.clear();
}

/// Drop the status line unconditionally (decay timer in QML).
pub fn dismiss_status(state: &mut State) {
    state.status_text.clear();
}

/// Reveal the cwd in the OS file browser.
pub fn open_in_explorer(state: &mut State) {
    let cwd = PathBuf::from(state.cwd.clone());
    if let Err(e) = places::open_in_explorer(&cwd) {
        let detail = support::short_detail(&e.to_string());
        state.status_text = format!("storey error: cannot open explorer ({detail})");
    }
}

/// Set cwd, clearing the search filter when the directory changes.
fn move_cwd(state: &mut State, text: &str) {
    if state.cwd != text {
        state.filter_query.clear();
        state.untagged_only = false;
    }
    state.cwd = text.to_string();
}

/// Report a blocking enter attempt without navigating (tested).
pub fn blocked_message(is_repo: bool) -> Option<String> {
    if is_repo {
        Some("storey error: Use file explorer instead".to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_navigation_inside_repo() {
        assert_eq!(
            blocked_message(true).as_deref(),
            Some("storey error: Use file explorer instead")
        );
        assert!(blocked_message(false).is_none());
    }
}
