//! Library invokables: shallow filter, untagged toggle,
//! file-view switch, and record/file selection for the tag editor.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::{
    state::{fail, sync_all, sync_detail},
    store::ffi,
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn set_filter(mut self: Pin<&mut Self>, query: QString) {
        self.as_mut().set_filter_query(query);
        self.as_mut().clear_status();
        self.as_mut().refresh();
    }

    pub(crate) fn toggle_untagged(mut self: Pin<&mut Self>) {
        let next = !*self.as_ref().untagged_only();
        self.as_mut().set_untagged_only(next);
        if next {
            // Untagged results only exist in the file view.
            self.as_mut().set_file_view(true);
        }
        self.as_mut().clear_status();
        self.as_mut().refresh();
    }

    pub(crate) fn show_file_view(mut self: Pin<&mut Self>, file: bool) {
        self.as_mut().set_file_view(file);
        if !file && *self.as_ref().untagged_only() {
            // Leaving the untagged results: restore the search lens.
            self.as_mut().set_untagged_only(false);
            self.as_mut().clear_status();
            let mut pin = self.as_mut();
            sync_all(&mut pin);
        }
    }

    /// Materialize a new view key order (`view --tags=a,b`): covers
    /// breadcrumb add and per-pill remove from QML in one call.
    pub(crate) fn set_view_tags(mut self: Pin<&mut Self>, tags_json: QString) {
        let tags = parse_tags(&support::s(&tags_json));
        let cwd = support::path_of(self.as_ref().cwd());
        let client = support::client_of(self.as_ref().storey_bin());
        match client.set_view(&cwd, &tags, |_| {}) {
            Ok(_) => {
                let mut pin = self.as_mut();
                pin.as_mut().set_status_text(support::qstr(""));
                sync_all(&mut pin);
            }
            Err(e) => {
                let mut pin = self.as_mut();
                fail(&mut pin, "view", &e);
            }
        }
    }

    pub(crate) fn select_record(mut self: Pin<&mut Self>, id: QString) {
        self.as_mut().set_selected_id(id);
        self.as_mut().set_suggestions_json(support::qstr("[]"));
        self.as_mut().rust_mut().suggestions.clear();
        let mut pin = self.as_mut();
        sync_detail(&mut pin);
    }

    pub(crate) fn select_file(mut self: Pin<&mut Self>, path: QString) {
        let target = support::s(&path);
        let id = resolve_id(self.as_ref(), &target);
        self.as_mut().set_selected_id(support::qstr(&id));
        self.as_mut().set_suggestions_json(support::qstr("[]"));
        self.as_mut().rust_mut().suggestions.clear();
        let mut pin = self.as_mut();
        sync_detail(&mut pin);
    }
}

/// Resolve an explorer path to a record id via display name match.
fn resolve_id(obj: Pin<&ffi::StoreyLibrary>, target: &str) -> String {
    let base = std::path::Path::new(target)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();
    obj.rust()
        .records
        .iter()
        .find(|r| r.display_name == base)
        .map(|r| r.id.clone())
        .unwrap_or_default()
}

/// Parse the QML view-tag array; garbage becomes an empty view.
fn parse_tags(raw: &str) -> Vec<String> {
    serde_json::from_str(raw).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tag_arrays() {
        assert_eq!(parse_tags(r#"["a","b"]"#), vec!["a", "b"]);
        assert!(parse_tags("garbage").is_empty());
    }
}
