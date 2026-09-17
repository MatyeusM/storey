//! Browse rendering: top nav bar, dir/file view, control bar.
//!
//! Layout is fixed height so mouse clicks map from one stored width:
//! row 1 is the button row (`[UP]` at columns 1-4, breadcrumbs in
//! libraries, `[Init]`/`[Files]` pinned right), row 4 is the first
//! file row, and the last row is the unframed control line.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    widgets::Paragraph,
};

use crate::{
    app::{App, Mode},
    browse, controls, detail_view, dialog, error, files, filter, manage, plugins_view, tagbar,
    topbar::{self, RIGHT_WIDTH, UP_MAX_COL},
};

/// Row of the button line inside the top block (0-based, terminal).
const BUTTON_ROW: u16 = 1;
/// First file row (top block is 3 tall, list border adds 1).
const LIST_FIRST_ROW: u16 = 4;

/// Draw nav bar, folder/file view, and the one-line control bar.
pub fn draw(f: &mut ratatui::Frame<'_>, app: &mut App) {
    let area = f.area();
    app.bar_width = area.width;
    app.term_w = area.width;
    app.term_h = area.height;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(area);
    f.render_widget(topbar::top_bar(app, app.bar_width), chunks[0]);
    match app.mode {
        Mode::Browse => f.render_widget(browse::file_list(app), chunks[1]),
        Mode::Files => f.render_widget(files::render_files(app), chunks[1]),
        Mode::Detail => detail_view::render_detail(f, app, chunks[1]),
        Mode::Plugins => f.render_widget(plugins_view::render_plugins(app), chunks[1]),
    }
    let prog = app.bar_progress();
    f.render_widget(
        Paragraph::new(controls::for_app(app).render(&app.notice, prog.as_ref(), app.bar_width)),
        chunks[2],
    );
    if app.add_open {
        dialog::render(f, app, &app.available_tags());
    }
    if app.mode == Mode::Files && app.files.filter.open {
        filter::render_popup(f, app);
    }
    if app.mode == Mode::Files && app.files.rm_open {
        manage::render_rm_popup(f, app);
    }
    if app.mode == Mode::Files && app.files.rename.is_some() {
        manage::render_rename_popup(f, app);
    }
    if let Some(err) = app.error.clone() {
        error::render(f, &err);
    }
}

/// Route a left-click: error modal, dialog rows, chips, files.
pub fn click(app: &mut App, col: u16, row: u16) {
    if app.error.is_some() {
        app.close_error();
        return;
    }
    if app.add_open {
        click_dialog(app, col, row);
        return;
    }
    if app.mode == Mode::Files && files_popup_open(app) {
        app.close_filter();
        app.close_rm();
        app.close_rename();
        return;
    }
    if row == BUTTON_ROW {
        if click_right_button(app, col) {
            return;
        }
        if (1..=UP_MAX_COL).contains(&col) {
            app.up();
            return;
        }
        if app.is_repo {
            if let Some(idx) = chip_index(app, col) {
                app.tag_sel = idx;
                app.activate_tag(idx);
            }
            return;
        }
    }
    if app.mode == Mode::Browse {
        if row >= LIST_FIRST_ROW {
            app.click_row((row - LIST_FIRST_ROW) as usize);
        }
    } else if app.mode == Mode::Files && row >= LIST_FIRST_ROW {
        app.click_file((row - LIST_FIRST_ROW) as usize);
    }
}

fn click_dialog(app: &mut App, col: u16, row: u16) {
    let choices = app.available_tags();
    let area = Rect {
        x: 0,
        y: 0,
        width: app.term_w,
        height: app.term_h,
    };
    let Some(rect) = dialog::dialog_rect(area, choices.len()) else {
        app.close_add();
        return;
    };
    match dialog::row_at(rect, col, row, choices.len()) {
        Some(idx) => {
            app.add_sel = idx;
            app.confirm_add();
        }
        None => {
            if inside(rect, col, row) {
                return;
            }
            app.close_add();
        }
    }
}

fn inside(rect: Rect, col: u16, row: u16) -> bool {
    col >= rect.x && col < rect.x + rect.width && row >= rect.y && row < rect.y + rect.height
}

fn click_right_button(app: &mut App, col: u16) -> bool {
    let edge = app.bar_width.saturating_sub(1);
    if col >= edge.saturating_sub(RIGHT_WIDTH + 1) && col < edge {
        if app.is_repo {
            app.toggle_mode();
        } else {
            app.spawn_init();
        }
        return true;
    }
    false
}

fn chip_index(app: &App, col: u16) -> Option<usize> {
    tagbar::chip_at(&topbar::chip_ranges(app), col)
}

/// True when any files-view popup covers the list.
fn files_popup_open(app: &App) -> bool {
    app.files.filter.open || app.files.rm_open || app.files.rename.is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::App;
    use crate::fs::list_dir;
    use crossterm::event::{KeyCode, KeyModifiers};
    use std::path::PathBuf;

    fn tmp_app() -> (TempGuard, App) {
        use std::sync::atomic::{AtomicU64, Ordering};
        static N: AtomicU64 = AtomicU64::new(0);
        let n = N.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "storey-tui-{}-{}-{}",
            std::process::id(),
            n,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(n as u128)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("sub")).unwrap();
        std::fs::write(dir.join("b.txt"), "b").unwrap();
        std::fs::write(dir.join("a.txt"), "a").unwrap();
        std::fs::write(dir.join(".hidden"), "h").unwrap();
        let mut app = App::new(PathBuf::from("no-such-storey-binary"), dir.clone());
        app.close_error();
        (TempGuard(dir), app)
    }

    struct TempGuard(PathBuf);

    impl Drop for TempGuard {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn list_skips_hidden_dirs_first_sorted() {
        let (_g, app) = tmp_app();
        let names: Vec<_> = app.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["sub", "a.txt", "b.txt"]);
    }

    #[test]
    fn quit_keys() {
        let (_g, mut app) = tmp_app();
        assert!(app.on_key(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(app.on_key(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.on_key(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!app.on_key(KeyCode::Enter, KeyModifiers::NONE));
    }

    #[test]
    fn enter_and_up_navigate_plain_dirs() {
        let (_g, mut app) = tmp_app();
        assert!(!app.is_repo);
        assert_eq!(app.entries[0].name, "sub");
        app.enter_selected();
        assert!(app.cwd.ends_with("sub"));
        app.up();
        assert!(!app.cwd.ends_with("sub"));
    }

    #[test]
    fn repo_blocks_descent_but_allows_up() {
        let (_g, mut app) = tmp_app();
        app.is_repo = true;
        let root = app.cwd.clone();
        app.enter_selected();
        assert_eq!(app.cwd, root);
        app.click_row(0);
        assert_eq!(app.cwd, root);
        app.up();
        assert_eq!(app.cwd, root.parent().unwrap().to_path_buf());
    }

    #[test]
    fn click_chip_removes_tag() {
        let (_g, mut app) = tmp_app();
        app.is_repo = true;
        app.view = vec!["T1".into()];
        app.categories = vec!["T1".into()];
        app.bar_width = 80;
        app.tag_sel = 0;
        let ranges = topbar::chip_ranges(&app);
        let before = app.view.clone();
        click(&mut app, ranges[0].0, BUTTON_ROW);
        assert_ne!(app.view, before);
    }

    #[test]
    fn missing_binary_notice_names_it() {
        let (_g, app) = tmp_app();
        assert!(!app.is_repo);
        assert!(
            app.notice.contains("no storey binary"),
            "notice was {:?}",
            app.notice
        );
    }

    #[test]
    fn click_row_opens_dir() {
        let (_g, mut app) = tmp_app();
        click(&mut app, 10, LIST_FIRST_ROW);
        assert!(app.cwd.ends_with("sub"));
    }

    #[test]
    fn list_dir_standalone() {
        let (_g, app) = tmp_app();
        let again = list_dir(&app.cwd);
        assert_eq!(again.len(), 3);
        assert!(app.mode == Mode::Browse);
    }
}
