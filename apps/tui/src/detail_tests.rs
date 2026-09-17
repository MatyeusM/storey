//! Detail-view tests (split from `detail.rs` for the file-size limit).

use super::*;
use crate::app::{App, Mode};

fn detail_app() -> App {
    let dir = std::env::temp_dir().join(format!("storey-tui-detail-{}", std::process::id()));
    let mut app = App::new(std::path::PathBuf::from("no-such-storey-binary"), dir);
    app.close_error();
    app.is_repo = true;
    app.mode = Mode::Detail;
    app.tagging.open("ab".into());
    app
}

#[test]
fn input_appends_and_backspaces_without_cursor() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut app = detail_app();
    app.tagging.focus = Some(Focus::Vals);
    app.on_key_detail(KeyCode::Char('x'), KeyModifiers::NONE);
    app.on_key_detail(KeyCode::Char(','), KeyModifiers::NONE);
    assert_eq!(app.tagging.vals, "x,");
    app.on_key_detail(KeyCode::Backspace, KeyModifiers::NONE);
    assert_eq!(app.tagging.vals, "x");
    app.on_key_detail(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert_eq!(app.tagging.vals, "x");
    app.on_key_detail(KeyCode::Left, KeyModifiers::NONE);
    assert_eq!(app.tagging.vals, "x");
}

#[test]
fn mode_toggle_and_unfocus() {
    use crossterm::event::{KeyCode, KeyModifiers};
    let mut app = detail_app();
    assert!(app.tagging.set_mode);
    app.on_key_detail(KeyCode::Char('m'), KeyModifiers::NONE);
    assert!(!app.tagging.set_mode);
    app.on_key_detail(KeyCode::Char('c'), KeyModifiers::NONE);
    assert_eq!(app.tagging.focus, Some(Focus::Cat));
    app.on_key_detail(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.tagging.focus, None);
    app.on_key_detail(KeyCode::Esc, KeyModifiers::NONE);
    assert_eq!(app.mode, Mode::Files);
}

#[test]
fn suggest_with_no_plugins_notices() {
    use std::collections::BTreeMap;
    let mut app = detail_app();
    app.files.records = vec![storey_shared::model::Record {
        id: "ab".into(),
        display_name: "a.mkv".into(),
        original_rel: "a.mkv".into(),
        stored_rel: String::new(),
        size: 1,
        tags: BTreeMap::new(),
    }];
    app.do_suggest();
    assert!(!app.tagging.suggest_open);
    assert_eq!(app.notice, "no suggestions");
}

#[test]
fn apply_tags_refreshes_record_and_resets_buffers() {
    let bin = std::env::var("STOREY_BIN").unwrap_or_default();
    if !std::path::Path::new(&bin).is_file() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("storey-tui-apply-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("f.mkv"), "data").unwrap();
    for args in [["init"], ["update"]] {
        let out = std::process::Command::new(&bin)
            .args(args)
            .current_dir(&dir)
            .output()
            .unwrap();
        assert!(out.status.success());
    }
    let mut app = App::new(std::path::PathBuf::from(&bin), dir.clone());
    app.close_error();
    app.enter_files();
    assert_eq!(app.files.records.len(), 1);
    app.open_detail();
    app.tagging.cat = "genre".into();
    app.tagging.vals = "rock,pop".into();
    app.apply_tags();
    let rec = app.detail_record().cloned().expect("record gone");
    assert_eq!(
        rec.tags.get("genre").cloned().unwrap_or_default(),
        vec!["rock".to_string(), "pop".to_string()]
    );
    assert!(app.tagging.cat.is_empty() && app.tagging.vals.is_empty());
    assert!(app.tagging.focus.is_none());
    let _ = std::fs::remove_dir_all(&dir);
}
