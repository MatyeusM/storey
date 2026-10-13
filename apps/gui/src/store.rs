//! storey-gui: Qt6 graphical interface over linked `core`.
//!
//! The QObject below is a thin QString mirror: every invokable
//! moves the inner [`ui::State`] out, calls one `ui` action, and
//! commits the result back for QML. All logic lives in `crates/ui`.

use cxx_qt_lib::QString;

use core::pin::Pin;

use cxx_qt::CxxQtType;

#[cxx_qt::bridge]
pub mod ffi {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    extern "RustQt" {
        /// Library handle exposed to Qt/QML (properties next).
        #[qobject]
        #[qml_element]
        #[qproperty(QString, gui_overrides)]
        #[qproperty(QString, cwd)]
        #[qproperty(QString, root)]
        #[qproperty(QString, status_text)]
        #[qproperty(QString, entries_json)]
        #[qproperty(QString, records_json)]
        #[qproperty(QString, view_json)]
        #[qproperty(QString, categories_json)]
        #[qproperty(QString, detail_json)]
        #[qproperty(QString, suggestions_json)]
        #[qproperty(QString, plugins_json)]
        #[qproperty(QString, places_json)]
        #[qproperty(QString, filter_query)]
        #[qproperty(QString, selected_id)]
        #[qproperty(QString, progress_label)]
        #[qproperty(QString, size_text)]
        #[qproperty(bool, is_repo)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, file_view)]
        #[qproperty(bool, untagged_only)]
        #[qproperty(i32, folder_count)]
        #[qproperty(i32, file_count)]
        #[qproperty(f64, progress_fraction)]
        type StoreyLibrary = super::StoreyLibraryRust;

        #[qinvokable]
        fn refresh(self: Pin<&mut Self>);
        #[qinvokable]
        fn go_up(self: Pin<&mut Self>);
        #[qinvokable]
        fn open_path(self: Pin<&mut Self>, path: QString);
        #[qinvokable]
        fn enter_dir(self: Pin<&mut Self>, path: QString);
        #[qinvokable]
        fn run_init(self: Pin<&mut Self>);
        #[qinvokable]
        fn run_update(self: Pin<&mut Self>);
        #[qinvokable]
        fn open_in_explorer(self: Pin<&mut Self>);
        #[qinvokable]
        fn set_filter(self: Pin<&mut Self>, query: QString);
        #[qinvokable]
        fn toggle_untagged(self: Pin<&mut Self>);
        #[qinvokable]
        fn show_file_view(self: Pin<&mut Self>, file: bool);
        #[qinvokable]
        fn set_view_tags(self: Pin<&mut Self>, tags_json: QString);
        #[qinvokable]
        fn select_record(self: Pin<&mut Self>, id: QString);
        #[qinvokable]
        fn select_file(self: Pin<&mut Self>, path: QString);
        #[qinvokable]
        fn apply_tags(self: Pin<&mut Self>, category: QString, values: QString);
        #[qinvokable]
        fn do_suggest(self: Pin<&mut Self>);
        #[qinvokable]
        fn apply_suggestions(self: Pin<&mut Self>);
        #[qinvokable]
        fn reload_plugins(self: Pin<&mut Self>);
        #[qinvokable]
        fn add_plugin(self: Pin<&mut Self>, url: QString);
        #[qinvokable]
        fn set_plugin_enabled(self: Pin<&mut Self>, index: i32, enabled: bool);
        #[qinvokable]
        fn clear_status(self: Pin<&mut Self>);
        #[qinvokable]
        fn dismiss_status(self: Pin<&mut Self>);
    }

    impl cxx_qt::Threading for StoreyLibrary {}
}

/// Rust state backing the [`ffi::StoreyLibrary`] QObject: QString
/// mirror plus the logic-owned [`ui::State`].
pub struct StoreyLibraryRust {
    gui_overrides: QString,
    cwd: QString,
    root: QString,
    status_text: QString,
    entries_json: QString,
    records_json: QString,
    view_json: QString,
    categories_json: QString,
    detail_json: QString,
    suggestions_json: QString,
    plugins_json: QString,
    places_json: QString,
    filter_query: QString,
    selected_id: QString,
    progress_label: QString,
    size_text: QString,
    is_repo: bool,
    busy: bool,
    file_view: bool,
    untagged_only: bool,
    folder_count: i32,
    file_count: i32,
    progress_fraction: f64,
    pub(crate) inner: ui::State,
}

impl Default for StoreyLibraryRust {
    /// Start dir from the environment; QML calls `refresh` next.
    /// Theme defaults live in QML — only explicit, valid `[gui]`
    /// keys travel in `gui_overrides`.
    fn default() -> Self {
        let settings = ui::settings::load().unwrap_or_default();
        let cwd = std::env::var("STOREY_START_DIR").unwrap_or_else(|_| ".".into());
        let inner = ui::State::new(cwd, settings);
        Self {
            gui_overrides: Self::q(&inner.gui_overrides),
            cwd: Self::q(&inner.cwd),
            root: Self::q(&inner.root),
            status_text: Self::q(&inner.status_text),
            entries_json: Self::q(&inner.entries_json),
            records_json: Self::q(&inner.records_json),
            view_json: Self::q(&inner.view_json),
            categories_json: Self::q(&inner.categories_json),
            detail_json: Self::q(&inner.detail_json),
            suggestions_json: Self::q(&inner.suggestions_json),
            plugins_json: Self::q(&inner.plugins_json),
            places_json: Self::q(&inner.places_json),
            filter_query: Self::q(&inner.filter_query),
            selected_id: Self::q(&inner.selected_id),
            progress_label: Self::q(&inner.progress_label),
            size_text: Self::q(&inner.size_text),
            is_repo: inner.is_repo,
            busy: inner.busy,
            file_view: inner.file_view,
            untagged_only: inner.untagged_only,
            folder_count: inner.folder_count,
            file_count: inner.file_count,
            progress_fraction: inner.progress_fraction,
            inner,
        }
    }
}

impl StoreyLibraryRust {
    /// Wrap a string for a `QString` property.
    pub(crate) fn q(value: &str) -> QString {
        QString::from(value)
    }
}

/// Move the logic state out (callers commit it back).
pub(crate) fn take_state(mut obj: Pin<&mut ffi::StoreyLibrary>) -> ui::State {
    std::mem::take(&mut obj.as_mut().rust_mut().inner)
}

/// Mirror logic state into QString properties, then store it.
pub(crate) fn commit(mut obj: Pin<&mut ffi::StoreyLibrary>, st: ui::State) {
    obj.as_mut()
        .set_gui_overrides(StoreyLibraryRust::q(&st.gui_overrides));
    obj.as_mut().set_cwd(StoreyLibraryRust::q(&st.cwd));
    obj.as_mut().set_root(StoreyLibraryRust::q(&st.root));
    obj.as_mut()
        .set_status_text(StoreyLibraryRust::q(&st.status_text));
    obj.as_mut()
        .set_entries_json(StoreyLibraryRust::q(&st.entries_json));
    obj.as_mut()
        .set_records_json(StoreyLibraryRust::q(&st.records_json));
    obj.as_mut()
        .set_view_json(StoreyLibraryRust::q(&st.view_json));
    obj.as_mut()
        .set_categories_json(StoreyLibraryRust::q(&st.categories_json));
    obj.as_mut()
        .set_detail_json(StoreyLibraryRust::q(&st.detail_json));
    obj.as_mut()
        .set_suggestions_json(StoreyLibraryRust::q(&st.suggestions_json));
    obj.as_mut()
        .set_plugins_json(StoreyLibraryRust::q(&st.plugins_json));
    obj.as_mut()
        .set_places_json(StoreyLibraryRust::q(&st.places_json));
    obj.as_mut()
        .set_filter_query(StoreyLibraryRust::q(&st.filter_query));
    obj.as_mut()
        .set_selected_id(StoreyLibraryRust::q(&st.selected_id));
    obj.as_mut()
        .set_progress_label(StoreyLibraryRust::q(&st.progress_label));
    obj.as_mut()
        .set_size_text(StoreyLibraryRust::q(&st.size_text));
    obj.as_mut().set_is_repo(st.is_repo);
    obj.as_mut().set_busy(st.busy);
    obj.as_mut().set_file_view(st.file_view);
    obj.as_mut().set_untagged_only(st.untagged_only);
    obj.as_mut().set_folder_count(st.folder_count);
    obj.as_mut().set_file_count(st.file_count);
    obj.as_mut().set_progress_fraction(st.progress_fraction);
    obj.as_mut().rust_mut().inner = st;
}

/// Run one logic action against the inner state, then mirror.
pub(crate) fn act(mut obj: Pin<&mut ffi::StoreyLibrary>, f: impl FnOnce(&mut ui::State)) {
    let mut st = take_state(obj.as_mut());
    f(&mut st);
    commit(obj.as_mut(), st);
}
