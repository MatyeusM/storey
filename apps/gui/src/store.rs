//! storey-gui: Qt6 graphical interface to the `storey` CLI.
//!
//! [`ffi::StoreyLibrary`] is the QML-facing root object: Win11-style
//! explorer state (cwd, filtered entries/records, counts, plugins)
//! plus the explicit `ui.toml` `[gui]` overrides as JSON. Bodies live
//! in `nav`, `library`, `tagging`, `places`, and `plugins` so the
//! bridge file stays small. Library work goes via `storey-shared`.

use cxx_qt_lib::QString;
use storey_shared::{model::Record, settings::Settings, tags::Suggestion};

use crate::plugins::PluginRow;

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
        #[qproperty(QString, storey_bin)]
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
        fn configure(self: Pin<&mut Self>, bin: QString, cwd: QString);
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

/// Rust state backing the [`ffi::StoreyLibrary`] QObject.
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
    storey_bin: QString,
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
    pub(crate) records: Vec<Record>,
    pub(crate) view: Vec<String>,
    pub(crate) categories: Vec<String>,
    pub(crate) suggestions: Vec<Suggestion>,
    pub(crate) plugin_rows: Vec<PluginRow>,
    pub(crate) settings: Settings,
}

impl Default for StoreyLibraryRust {
    /// Env-configured paths plus explicit `ui.toml` overrides; QML
    /// calls `refresh` next. Theme defaults live in QML — only
    /// explicit, valid `[gui]` keys travel in `gui_overrides`.
    fn default() -> Self {
        let settings = storey_shared::settings::load().unwrap_or_default();
        let json = storey_shared::theme::GuiTheme::overrides_json(&settings.gui);
        let bin = std::env::var("STOREY_BIN").unwrap_or_else(|_| "storey".into());
        let cwd = std::env::var("STOREY_START_DIR").unwrap_or_else(|_| ".".into());
        Self {
            gui_overrides: Self::q(&json.to_string()),
            cwd: Self::q(&cwd),
            root: Self::q(""),
            status_text: Self::q(""),
            entries_json: Self::q("[]"),
            records_json: Self::q("[]"),
            view_json: Self::q("[]"),
            categories_json: Self::q("[]"),
            detail_json: Self::q("null"),
            suggestions_json: Self::q("[]"),
            plugins_json: Self::q("[]"),
            places_json: Self::q("[]"),
            filter_query: Self::q(""),
            storey_bin: Self::q(&bin),
            selected_id: Self::q(""),
            progress_label: Self::q(""),
            size_text: Self::q(""),
            is_repo: false,
            busy: false,
            file_view: false,
            untagged_only: false,
            folder_count: 0,
            file_count: 0,
            progress_fraction: -1.0,
            records: Vec::new(),
            view: Vec::new(),
            categories: Vec::new(),
            suggestions: Vec::new(),
            plugin_rows: Vec::new(),
            settings,
        }
    }
}

impl StoreyLibraryRust {
    /// Wrap a string for a `QString` property.
    pub(crate) fn q(value: &str) -> QString {
        QString::from(value)
    }
}
