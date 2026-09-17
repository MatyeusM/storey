//! Tag-editor invokables: set, suggestions, apply-all.
//!
//! Tag writes refresh the record list quietly; the view itself is
//! regenerable CLI output, so no view rebuild happens here.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;
use storey_shared::tags::{ExternalPlugin, TagSource, rank};

use crate::{
    state::{fail, sync_detail},
    store::ffi,
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn apply_tags(mut self: Pin<&mut Self>, category: QString, values: QString) {
        let cat = support::s(&category).trim().to_string();
        let vals = support::split_values(&support::s(&values));
        if cat.is_empty() || vals.is_empty() {
            let mut pin = self.as_mut();
            pin.as_mut().set_status_text(support::qstr(
                "storey error: enter a category and comma-separated values",
            ));
            return;
        }
        let id = support::s(self.as_ref().selected_id());
        let cwd = support::path_of(self.as_ref().cwd());
        let client = support::client_of(self.as_ref().storey_bin());
        let result = client.set_tags(&cwd, &id, &cat, &vals, |_| {});
        match result {
            Ok(_) => {
                let mut pin = self.as_mut();
                pin.as_mut().set_status_text(support::qstr(""));
                sync_records_quiet(&mut pin, &cwd);
            }
            Err(e) => {
                let mut pin = self.as_mut();
                fail(&mut pin, "tag", &e);
            }
        }
        let mut pin = self.as_mut();
        sync_detail(&mut pin);
    }

    pub(crate) fn do_suggest(mut self: Pin<&mut Self>) {
        let id = support::s(self.as_ref().selected_id());
        let root = support::path_of(self.as_ref().root());
        let (record, settings) = {
            let rust = self.rust();
            let record = rust.records.iter().find(|r| r.id == id).cloned();
            (record, rust.settings.clone())
        };
        let Some(record) = record else {
            let mut pin = self.as_mut();
            pin.as_mut()
                .set_status_text(support::qstr("storey error: select a file first"));
            return;
        };
        let all: Vec<_> = settings
            .enabled_plugins()
            .filter_map(|decl| ExternalPlugin::from_decl(decl).ok())
            .filter_map(|plugin| plugin.lookup(&record, &root).ok())
            .flatten()
            .collect();
        let ranked = rank(all);
        if ranked.is_empty() {
            let mut pin = self.as_mut();
            pin.as_mut()
                .set_status_text(support::qstr("no suggestions"));
            return;
        }
        self.as_mut().rust_mut().suggestions = ranked;
        let json = {
            let rust = self.rust();
            support::suggestions_json(&rust.suggestions)
        };
        self.as_mut().set_suggestions_json(support::qstr(&json));
    }

    pub(crate) fn apply_suggestions(mut self: Pin<&mut Self>) {
        use std::collections::BTreeMap;
        let id = support::s(self.as_ref().selected_id());
        let grouped: BTreeMap<String, Vec<String>> = {
            let rust = self.rust();
            let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
            for item in &rust.suggestions {
                let values = grouped.entry(item.category.clone()).or_default();
                if !values.contains(&item.value) {
                    values.push(item.value.clone());
                }
            }
            grouped
        };
        if grouped.is_empty() {
            return;
        }
        let cwd = support::path_of(self.as_ref().cwd());
        let client = support::client_of(self.as_ref().storey_bin());
        for (cat, values) in &grouped {
            if let Err(e) = client.add_tags(&cwd, &id, cat, values, |_| {}) {
                let mut pin = self.as_mut();
                fail(&mut pin, "tag", &e);
                return;
            }
        }
        self.as_mut().rust_mut().suggestions.clear();
        self.as_mut().set_suggestions_json(support::qstr("[]"));
        let mut pin = self.as_mut();
        pin.as_mut().set_status_text(support::qstr(""));
        sync_records_quiet(&mut pin, &cwd);
        sync_detail(&mut pin);
    }
}

/// Reload `list` rows and categories without touching the status
/// line. Categories must refresh here: the folder-view [+] button
/// reads them, and the first-ever tags would otherwise stay hidden
/// until the next navigation.
fn sync_records_quiet(obj: &mut Pin<&mut ffi::StoreyLibrary>, cwd: &std::path::Path) {
    let client = support::client_of(obj.as_ref().storey_bin());
    if let Ok(records) = client.list(cwd, |_| {}) {
        obj.as_mut().rust_mut().records = records;
    }
    if let Ok(cats) = client.categories(cwd, |_| {}) {
        obj.as_mut().rust_mut().categories = cats.clone();
        obj.as_mut()
            .set_categories_json(support::qstr(&support::str_list_json(&cats)));
    }
    let query = support::s(obj.as_ref().filter_query());
    let untagged = *obj.as_ref().untagged_only();
    let json = {
        let rust = obj.rust();
        support::records_json(&rust.records, &query, untagged)
    };
    obj.as_mut().set_records_json(support::qstr(&json));
}
