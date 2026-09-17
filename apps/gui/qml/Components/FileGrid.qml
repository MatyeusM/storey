import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Middle file grid: folders and files share one tile grid (folders
// first), except the files view inside a library shows records only.
// Inside a library the folders are view output — clicking one
// reports to the status bar instead of navigating; clicking a file
// opens the tag editor.
Frame {
    id: root

    property var library
    property var entries: []
    property var records: []
    property bool isRepo: false
    property bool fileView: true

    signal openTagEditor

    topPadding: Spacing.sm
    bottomPadding: Spacing.sm
    leftPadding: Spacing.sm
    rightPadding: Spacing.sm

    background: Rectangle {
        color: AppTheme.background
    }

    function tiles() {
        if (root.isRepo && root.fileView) {
            return root.records.map(function (r) {
                return {
                    "kind": "record",
                    "id": r.id,
                    "name": r.displayName,
                    "path": "",
                    "size": r.size
                };
            });
        }
        var dirs = root.entries.filter(function (e) {
            return e.isDir;
        }).map(function (e) {
            return {
                "kind": "dir",
                "id": "",
                "name": e.name,
                "path": e.path,
                "size": 0
            };
        });
        var files = root.entries.filter(function (e) {
            return !e.isDir;
        }).map(function (e) {
            return {
                "kind": "entry",
                "id": "",
                "name": e.name,
                "path": e.path,
                "size": e.size
            };
        });
        return dirs.concat(files);
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Spacing.xs

        Label {
            visible: root.isRepo && !root.fileView
            text: qsTr("tag view — files live in the file view")
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
        ScrollView {
            id: scroll
            Layout.fillWidth: true
            Layout.fillHeight: true
            clip: true

            ColumnLayout {
                // NOTE: children of a ScrollView are reparented to its
                // contentItem, so parent.width here is the content width
                // (which collapses) — bind to the ScrollView itself or
                // the grid shrinks to a single column.
                width: scroll.width
                spacing: Spacing.sm

                GridView {
                    id: tileGrid
                    Layout.fillWidth: true
                    Layout.preferredHeight: contentHeight
                    visible: count > 0
                    interactive: false
                    cellWidth: Base.rem(7)
                    cellHeight: Base.rem(6)
                    model: root.tiles()
                    delegate: Item {
                        required property var modelData
                        width: tileGrid.cellWidth
                        height: tileGrid.cellHeight
                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: Spacing.xxs
                            spacing: 2
                            Icon {
                                Layout.alignment: Qt.AlignHCenter
                                glyph: modelData.kind === "dir" ? Icons.folder : Icons.file
                                size: Base.rem(2.5)
                                color: modelData.kind === "dir" ? AppTheme.accent : AppTheme.muted
                            }
                            Label {
                                Layout.fillWidth: true
                                horizontalAlignment: Text.AlignHCenter
                                text: modelData.name
                                color: AppTheme.text
                                elide: Text.ElideMiddle
                                maximumLineCount: 2
                                wrapMode: Text.Wrap
                                font: Typography.font(Typography.sm)
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: (modelData.kind === "dir" || root.isRepo) ? Qt.PointingHandCursor : Qt.ArrowCursor
                            onClicked: {
                                if (modelData.kind === "dir") {
                                    root.library.enter_dir(modelData.path);
                                    return;
                                }
                                if (!root.isRepo)
                                    return;
                                if (modelData.kind === "record")
                                    root.library.select_record(modelData.id);
                                else
                                    root.library.select_file(modelData.path);
                                root.openTagEditor();
                            }
                        }
                    }
                }
                Label {
                    visible: tileGrid.count === 0
                    text: qsTr("empty folder")
                    color: AppTheme.muted
                    font: Typography.font(Typography.sm)
                }
            }
        }
    }
}
