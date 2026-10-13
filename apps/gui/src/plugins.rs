//! Thin plugin-dialog invokables: each mirrors one `ui` state action.

use core::pin::Pin;

use cxx_qt_lib::QString;

use crate::{
    store::{act, ffi},
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn reload_plugins(mut self: Pin<&mut Self>) {
        act(self.as_mut(), ui::state::State::reload_plugins);
    }

    pub(crate) fn set_plugin_enabled(mut self: Pin<&mut Self>, index: i32, enabled: bool) {
        act(self.as_mut(), move |st| {
            st.set_plugin_enabled(index as usize, enabled)
        });
    }

    pub(crate) fn add_plugin(mut self: Pin<&mut Self>, url: QString) {
        let url = support::s(&url);
        act(self.as_mut(), move |st| st.add_plugin(&url));
    }
}
