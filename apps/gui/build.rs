use cxx_qt_build::{CxxQtBuilder, QResource, QResources, QmlFile, QmlModule};

/// QML sources compiled into the binary (module files, no singletons).
const QML_FILES: [&str; 12] = [
    "qml/Main.qml",
    "qml/Ui/Icon.qml",
    "qml/Ui/IconButton.qml",
    "qml/Ui/PrimaryButton.qml",
    "qml/Components/TopBar.qml",
    "qml/Components/SidePane.qml",
    "qml/Components/FileGrid.qml",
    "qml/Components/StatusBar.qml",
    "qml/Components/ConfirmDialog.qml",
    "qml/Components/PluginsDialog.qml",
    "qml/Components/TagEditorDialog.qml",
    "qml/Components/ViewTagDialog.qml",
];

/// Theme singletons plus fonts: watched for reruns, not in the module.
const QML_WATCH: [&str; 9] = [
    "qml/Theme/Base.qml",
    "qml/Theme/AppTheme.qml",
    "qml/Theme/Icons.qml",
    "qml/Theme/Spacing.qml",
    "qml/Theme/Typography.qml",
    "qml/Theme/fonts/PTSans-Regular.ttf",
    "qml/Theme/fonts/PTSans-Italic.ttf",
    "qml/Theme/fonts/PTSans-Bold.ttf",
    "qml/Theme/fonts/SymbolsNerdFont-Regular.ttf",
];

fn main() {
    // QML is compiled into the binary: watch every source file so
    // cargo reruns the build script when the UI changes.
    for file in QML_FILES.into_iter().chain(QML_WATCH) {
        println!("cargo::rerun-if-changed={file}");
    }
    let singletons = ["Base", "AppTheme", "Icons", "Spacing", "Typography"]
        .into_iter()
        .map(|name| QmlFile::from(format!("qml/Theme/{name}.qml")).singleton(true));
    let qml_module = QmlModule::new("com.storey.gui")
        .qml_files(QML_FILES)
        .qml_files(singletons);
    let fonts = QResource::new().files([
        "qml/Theme/fonts/PTSans-Regular.ttf",
        "qml/Theme/fonts/PTSans-Italic.ttf",
        "qml/Theme/fonts/PTSans-Bold.ttf",
        "qml/Theme/fonts/SymbolsNerdFont-Regular.ttf",
    ]);
    CxxQtBuilder::new_qml_module(qml_module)
        .file("src/store.rs")
        .qrc_resources(QResources::new().resource(fonts))
        .build();
}
