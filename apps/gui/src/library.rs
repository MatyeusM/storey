//! Thin library invokables: each mirrors one `storey_ui::library` action.

use core::pin::Pin;

use cxx_qt_lib::QString;

use crate::{
    store::{act, ffi},
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn set_filter(mut self: Pin<&mut Self>, query: QString) {
        let query = support::s(&query);
        act(self.as_mut(), move |st| {
            storey_ui::library::set_filter(st, query)
        });
    }

    pub(crate) fn toggle_untagged(mut self: Pin<&mut Self>) {
        act(self.as_mut(), storey_ui::library::toggle_untagged);
    }

    pub(crate) fn show_file_view(mut self: Pin<&mut Self>, file: bool) {
        act(self.as_mut(), move |st| {
            storey_ui::library::show_file_view(st, file)
        });
    }

    pub(crate) fn set_view_tags(mut self: Pin<&mut Self>, tags_json: QString) {
        let raw = support::s(&tags_json);
        act(self.as_mut(), move |st| {
            storey_ui::library::set_view_tags(st, &raw)
        });
    }

    pub(crate) fn select_record(mut self: Pin<&mut Self>, id: QString) {
        let id = support::s(&id);
        act(self.as_mut(), move |st| {
            storey_ui::library::select_record(st, id)
        });
    }

    pub(crate) fn select_file(mut self: Pin<&mut Self>, path: QString) {
        let target = support::s(&path);
        act(self.as_mut(), move |st| {
            storey_ui::library::select_file(st, &target)
        });
    }
}
