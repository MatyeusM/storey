//! Key handling for [`App`]: modal-first dispatch across views.
//!
//! Error popup, then the add-tag dialog, then the current view:
//! repo breadcrumbs vs plain browsing in `Browse`, and the `Files`,
//! `Detail`, and `Plugins` handlers living in their own modules.

use crossterm::event::{KeyCode, KeyModifiers};

use crate::app::{App, Mode};

impl App {
    /// Handle one key press, reporting whether to quit.
    ///
    /// Dispatch is modal-first: error popup, add dialog, then the
    /// current view. `Esc`/`q` quit only from Browse; the other
    /// views use them for back navigation.
    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        if self.error.is_some() {
            return self.on_key_error(code, mods);
        }
        if self.add_open {
            return self.on_key_dialog(code, mods);
        }
        match self.mode {
            Mode::Browse => {
                if quit_key(code, mods) {
                    return true;
                }
                if self.is_repo {
                    self.on_key_repo(code);
                } else {
                    self.on_key_plain(code);
                }
                false
            }
            Mode::Files => self.on_key_files(code, mods),
            Mode::Detail => self.on_key_detail(code, mods),
            Mode::Plugins => self.on_key_plugins(code, mods),
        }
    }

    /// A storey error is modal: dismiss keys close it, the rest
    /// is swallowed so background state cannot change underneath.
    fn on_key_error(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        match code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char(' ') => self.close_error(),
            KeyCode::Char('q') if mods == KeyModifiers::NONE => self.close_error(),
            _ => {}
        }
        false
    }

    /// Keys while the add-tag dialog is open.
    fn on_key_dialog(&mut self, code: KeyCode, mods: KeyModifiers) -> bool {
        match code {
            KeyCode::Esc => self.close_add(),
            KeyCode::Char('q') if mods == KeyModifiers::NONE => self.close_add(),
            KeyCode::Up | KeyCode::Char('k') => self.move_add_sel(-1),
            KeyCode::Down | KeyCode::Char('j') => self.move_add_sel(1),
            KeyCode::Enter => self.confirm_add(),
            _ => {}
        }
        false
    }

    /// Library mode: arrows/`hjkl` move between breadcrumb chips.
    fn on_key_repo(&mut self, code: KeyCode) {
        match code {
            KeyCode::Left | KeyCode::Right => self.step_tag(code),
            KeyCode::Up | KeyCode::Down => self.step_tag(code),
            KeyCode::Char('h') | KeyCode::Char('l') => self.step_tag(code),
            KeyCode::Char('j') | KeyCode::Char('k') => self.step_tag(code),
            KeyCode::Enter | KeyCode::Char(' ') => self.activate_selected_tag(),
            KeyCode::Backspace | KeyCode::Char('u') => self.up(),
            KeyCode::Delete | KeyCode::Char('x') => self.remove_selected_tag(),
            KeyCode::Char('a') | KeyCode::Char('+') => self.open_add(),
            KeyCode::Char('t') => self.toggle_mode(),
            KeyCode::Char('r') => self.spawn_restore(),
            _ => {}
        }
    }

    /// Plain-directory browsing keeps the old file-list keys.
    /// `t` is repo-only (see [`App::enter_files`]), so plain dirs ignore it.
    fn on_key_plain(&mut self, code: KeyCode) {
        match code {
            KeyCode::Down | KeyCode::Char('j') => self.move_sel(1),
            KeyCode::Up | KeyCode::Char('k') => self.move_sel(-1),
            KeyCode::Enter => self.enter_selected(),
            KeyCode::Backspace => self.up(),
            KeyCode::Char('u') => self.up(),
            KeyCode::Char('i') if !self.is_repo => self.spawn_init(),
            _ => {}
        }
    }

    fn step_tag(&mut self, code: KeyCode) {
        let delta = match code {
            KeyCode::Left | KeyCode::Up | KeyCode::Char('h') | KeyCode::Char('k') => -1,
            _ => 1,
        };
        self.move_tag(delta);
    }

    pub(crate) fn move_sel(&mut self, delta: i32) {
        if self.entries.is_empty() {
            return;
        }
        let len = self.entries.len() as i32;
        let next = (self.selected as i32 + delta).clamp(0, len - 1);
        self.selected = next as usize;
    }
}

fn quit_key(code: KeyCode, mods: KeyModifiers) -> bool {
    matches!(
        (code, mods),
        (KeyCode::Char('q'), KeyModifiers::NONE)
            | (KeyCode::Esc, _)
            | (KeyCode::Char('c'), KeyModifiers::CONTROL)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;

    fn repo_app() -> App {
        let dir = std::env::temp_dir().join(format!("storey-keys-{}", std::process::id()));
        let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
        app.close_error();
        app.is_repo = true;
        app.view = vec!["T1".into(), "T2".into()];
        app.categories = vec!["T1".into(), "T2".into(), "T3".into()];
        app.tag_sel = 0;
        app
    }

    #[test]
    fn repo_arrows_move_between_chips() {
        let mut app = repo_app();
        app.on_key(KeyCode::Char('l'), KeyModifiers::NONE);
        assert_eq!(app.tag_sel, 1);
        app.on_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(app.tag_sel, 2);
        app.on_key(KeyCode::Char('j'), KeyModifiers::NONE);
        assert_eq!(app.tag_sel, 0);
    }

    #[test]
    fn dialog_keys_navigate_without_quit() {
        let mut app = repo_app();
        app.open_add();
        assert!(app.add_open);
        assert!(!app.on_key(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!app.add_open);
    }
}
