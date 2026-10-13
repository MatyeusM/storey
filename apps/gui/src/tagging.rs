//! Thin tag-editor invokables: each mirrors one `ui::tagging` action.

use core::pin::Pin;

use cxx_qt_lib::QString;

use crate::{
    store::{act, ffi},
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn apply_tags(mut self: Pin<&mut Self>, category: QString, values: QString) {
        let cat = support::s(&category);
        let vals = support::s(&values);
        act(self.as_mut(), move |st| {
            ui::tagging::apply_tags(st, &cat, &vals)
        });
    }

    pub(crate) fn do_suggest(mut self: Pin<&mut Self>) {
        act(self.as_mut(), ui::tagging::do_suggest);
    }

    pub(crate) fn apply_suggestions(mut self: Pin<&mut Self>) {
        act(self.as_mut(), ui::tagging::apply_suggestions);
    }
}
