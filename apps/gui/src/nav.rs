//! Navigation invokables: configure, refresh, up/open/enter,
//! init/update on a worker thread, open-in-explorer, status clearing.
//!
//! Entering a directory inside a library is blocked on purpose:
//! the view is regenerable output, so drilling in would only
//! confuse — the status bar says to use the file explorer instead.

use core::pin::Pin;

use cxx_qt_lib::QString;
use storey_shared::cli::CliError;

use crate::{
    places,
    state::{finish_init, finish_update, send_tick, sync_all, take_busy},
    store::ffi,
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn configure(mut self: Pin<&mut Self>, bin: QString, cwd: QString) {
        if !support::s(&bin).is_empty() {
            self.as_mut().set_storey_bin(bin);
        }
        if !support::s(&cwd).is_empty() {
            self.as_mut().set_cwd(cwd);
        }
        self.as_mut().refresh();
    }

    pub(crate) fn refresh(mut self: Pin<&mut Self>) {
        let mut pin = self.as_mut();
        sync_all(&mut pin);
    }

    pub(crate) fn go_up(mut self: Pin<&mut Self>) {
        let cwd = support::path_of(self.as_ref().cwd());
        if let Some(parent) = cwd.parent() {
            let text = parent.display().to_string();
            move_cwd(self.as_mut(), &text);
        }
        self.as_mut().refresh();
    }

    pub(crate) fn open_path(mut self: Pin<&mut Self>, path: QString) {
        let target = support::path_of(&path);
        if target.is_dir() {
            let text = target.display().to_string();
            move_cwd(self.as_mut(), &text);
            self.as_mut().clear_status();
            self.as_mut().refresh();
        }
    }

    pub(crate) fn enter_dir(mut self: Pin<&mut Self>, path: QString) {
        if let Some(msg) = blocked_message(*self.as_ref().is_repo()) {
            self.as_mut().set_status_text(support::qstr(&msg));
            return;
        }
        let target = support::path_of(&path);
        if target.is_dir() {
            let text = target.display().to_string();
            move_cwd(self.as_mut(), &text);
            self.as_mut().refresh();
        }
    }

    pub(crate) fn clear_status(mut self: Pin<&mut Self>) {
        let kept = support::s(self.as_ref().status_text());
        if kept.starts_with("storey error:") || kept.starts_with("storey error (") {
            return;
        }
        self.as_mut().set_status_text(support::qstr(""));
    }

    /// Drop the status line unconditionally (decay timer in QML).
    pub(crate) fn dismiss_status(mut self: Pin<&mut Self>) {
        self.as_mut().set_status_text(support::qstr(""));
    }

    pub(crate) fn open_in_explorer(mut self: Pin<&mut Self>) {
        let cwd = support::path_of(self.as_ref().cwd());
        if let Err(e) = places::open_in_explorer(&cwd) {
            let detail = short_detail(&e.to_string());
            self.as_mut().set_status_text(support::qstr(&format!(
                "storey error: cannot open explorer ({detail})"
            )));
        }
    }

    pub(crate) fn run_init(mut self: Pin<&mut Self>) {
        use cxx_qt::Threading;
        if take_busy(self.as_mut()) {
            return;
        }
        let thread = self.as_ref().qt_thread();
        let bin = support::path_of(self.as_ref().storey_bin());
        let cwd = support::path_of(self.as_ref().cwd());
        std::thread::spawn(move || {
            let client = storey_shared::Client::new(bin);
            let result = client.call(&["init"], &cwd, |ev| {
                send_tick(&thread, "init", ev);
            });
            let done: Result<(), CliError> = result.map(|_| ());
            let _ = thread.queue(move |mut obj| {
                finish_init(&mut obj, done);
            });
        });
    }

    pub(crate) fn run_update(mut self: Pin<&mut Self>) {
        use cxx_qt::Threading;
        if take_busy(self.as_mut()) {
            return;
        }
        let thread = self.as_ref().qt_thread();
        let bin = support::path_of(self.as_ref().storey_bin());
        let cwd = support::path_of(self.as_ref().cwd());
        std::thread::spawn(move || {
            let client = storey_shared::Client::new(bin);
            let done = client.update(&cwd, false, |ev| {
                send_tick(&thread, "hash", ev);
            });
            let _ = thread.queue(move |mut obj| {
                finish_update(&mut obj, done);
            });
        });
    }
}

/// Set cwd, clearing the search filter when the directory changes.
fn move_cwd(mut obj: Pin<&mut ffi::StoreyLibrary>, text: &str) {
    if support::s(obj.as_ref().cwd()) != text {
        obj.as_mut().set_filter_query(support::qstr(""));
        obj.as_mut().set_untagged_only(false);
    }
    obj.as_mut().set_cwd(support::qstr(text));
}

/// Report a blocking enter attempt without navigating (tested).
pub(crate) fn blocked_message(is_repo: bool) -> Option<String> {
    if is_repo {
        Some("storey error: Use file explorer instead".to_string())
    } else {
        None
    }
}

/// Shorten a CLI error for status lines (tested).
pub(crate) fn short_detail(message: &str) -> String {
    let short = storey_shared::explore::short_err(message.to_string());
    if short.trim().is_empty() {
        "unknown error".to_string()
    } else {
        short
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

    #[test]
    fn shortens_details() {
        assert_eq!(short_detail("a\nb "), "b");
        assert_eq!(short_detail(""), "unknown error");
    }
}
