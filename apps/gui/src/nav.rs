//! Thin navigation invokables: each mirrors one `storey_ui::nav` action.
//!
//! Worker threads stay here (Qt thread queue); the flows they run
//! live in `ui`.

use core::pin::Pin;

use cxx_qt::CxxQtType;
use cxx_qt_lib::QString;

use crate::{
    store::{act, ffi},
    support,
};

impl ffi::StoreyLibrary {
    pub(crate) fn refresh(mut self: Pin<&mut Self>) {
        act(self.as_mut(), storey_ui::state::sync_all);
    }

    pub(crate) fn go_up(mut self: Pin<&mut Self>) {
        act(self.as_mut(), storey_ui::nav::go_up);
    }

    pub(crate) fn open_path(mut self: Pin<&mut Self>, path: QString) {
        let target = support::s(&path);
        act(self.as_mut(), move |st| {
            storey_ui::nav::open_path(st, &target)
        });
    }

    pub(crate) fn enter_dir(mut self: Pin<&mut Self>, path: QString) {
        let target = support::s(&path);
        act(self.as_mut(), move |st| {
            storey_ui::nav::enter_dir(st, &target)
        });
    }

    pub(crate) fn clear_status(mut self: Pin<&mut Self>) {
        act(self.as_mut(), storey_ui::nav::clear_status);
    }

    pub(crate) fn dismiss_status(mut self: Pin<&mut Self>) {
        act(self.as_mut(), storey_ui::nav::dismiss_status);
    }

    pub(crate) fn open_in_explorer(mut self: Pin<&mut Self>) {
        act(self.as_mut(), storey_ui::nav::open_in_explorer);
    }

    pub(crate) fn run_init(mut self: Pin<&mut Self>) {
        use cxx_qt::Threading;
        let cwd = self.as_ref().rust().inner.cwd.clone();
        let mut st = super::store::take_state(self.as_mut());
        if st.take_busy() {
            super::store::commit(self.as_mut(), st);
            return;
        }
        super::store::commit(self.as_mut(), st);
        let thread = self.as_ref().qt_thread();
        std::thread::spawn(move || {
            let logger = storey_core::log::Logger::new(false, false);
            let tick = thread.clone();
            logger.tap(move |ev| {
                if let Some((label, fraction)) = storey_ui::support::progress_label("init", &ev) {
                    let _ = tick.queue(move |mut obj| {
                        let mut st = super::store::take_state(obj.as_mut());
                        st.progress_label = label;
                        st.progress_fraction = fraction;
                        super::store::commit(obj.as_mut(), st);
                    });
                }
            });
            let done = storey_ui::nav::run_init_flow(&std::path::PathBuf::from(&cwd), &logger);
            let _ = thread.queue(move |mut obj| {
                let mut st = super::store::take_state(obj.as_mut());
                st.finish_init(done);
                super::store::commit(obj.as_mut(), st);
            });
        });
    }

    pub(crate) fn run_update(mut self: Pin<&mut Self>) {
        use cxx_qt::Threading;
        let cwd = self.as_ref().rust().inner.cwd.clone();
        let mut st = super::store::take_state(self.as_mut());
        if st.take_busy() {
            super::store::commit(self.as_mut(), st);
            return;
        }
        super::store::commit(self.as_mut(), st);
        let thread = self.as_ref().qt_thread();
        std::thread::spawn(move || {
            let logger = storey_core::log::Logger::new(false, false);
            let tick = thread.clone();
            logger.tap(move |ev| {
                if let Some((label, fraction)) = storey_ui::support::progress_label("hash", &ev) {
                    let _ = tick.queue(move |mut obj| {
                        let mut st = super::store::take_state(obj.as_mut());
                        st.progress_label = label;
                        st.progress_fraction = fraction;
                        super::store::commit(obj.as_mut(), st);
                    });
                }
            });
            let done = storey_ui::nav::run_update_flow(&std::path::PathBuf::from(&cwd), &logger);
            let _ = thread.queue(move |mut obj| {
                let mut st = super::store::take_state(obj.as_mut());
                st.finish_update(done);
                super::store::commit(obj.as_mut(), st);
            });
        });
    }
}
