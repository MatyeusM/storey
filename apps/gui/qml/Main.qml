import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Win11-style explorer shell: icon-only top bar, quick-access side
// pane, large-tile file grid, and a status footer under the grid.
// All library work goes through StoreyLibrary (storey CLI driver).
ApplicationWindow {
    id: root

    property var entries: JSON.parse(library.entries_json || "[]")
    property var records: JSON.parse(library.records_json || "[]")
    property var viewTags: JSON.parse(library.view_json || "[]")
    property var categories: JSON.parse(library.categories_json || "[]")
    property var detail: JSON.parse(library.detail_json || "null")
    property var suggestions: JSON.parse(library.suggestions_json || "[]")
    property var plugins: JSON.parse(library.plugins_json || "[]")
    property var places: JSON.parse(library.places_json || "[]")

    StoreyLibrary {
        id: library
    }

    Component.onCompleted: {
        Base.apply(library);
        AppTheme.apply(library);
        Typography.apply(library);
        library.refresh();
    }

    visible: true
    width: 1100
    height: 700
    title: qsTr("storey-gui")
    color: AppTheme.background

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        TopBar {
            Layout.fillWidth: true
            library: library
            viewTags: root.viewTags
            categories: root.categories
            fileView: library.file_view
            onAskInit: confirmDialog.ask(qsTr("Initialize Storey here? This creates an index in this folder."), function () {
                library.run_init();
            })
            onAskAddViewTag: viewTagDialog.openWith()
        }
        RowLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0

            SidePane {
                Layout.preferredWidth: Base.rem(11)
                Layout.fillHeight: true
                library: library
                places: root.places
                onOpenPlace: function (path) {
                    library.open_path(path);
                }
                onOpenPlugins: pluginsDialog.open()
            }
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 0

                FileGrid {
                    Layout.fillWidth: true
                    Layout.fillHeight: true
                    library: library
                    entries: root.entries
                    records: root.records
                    isRepo: library.is_repo
                    fileView: library.file_view
                    onOpenTagEditor: tagDialog.open()
                }
                StatusBar {
                    Layout.fillWidth: true
                    library: library
                }
            }
        }
    }

    ConfirmDialog {
        id: confirmDialog
    }
    PluginsDialog {
        id: pluginsDialog
        library: library
        plugins: root.plugins
        onAskAddPlugin: function (url) {
            confirmDialog.ask(qsTr("Install and run code from this URL? Only proceed if you trust the source:\n") + url, function () {
                library.add_plugin(url);
            });
        }
    }
    TagEditorDialog {
        id: tagDialog
        library: library
        detail: root.detail
        categories: root.categories
        suggestions: root.suggestions
    }
    ViewTagDialog {
        id: viewTagDialog
        library: library
        viewTags: root.viewTags
        categories: root.categories
    }
}
