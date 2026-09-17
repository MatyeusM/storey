import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Top command bar: icon-only actions plus the position display.
// Inside a library the position is "/Name" + a Files|Folders icon
// toggle plus the view-tag breadcrumbs; elsewhere it is the plain
// cwd. One row; the bottom hairline separates it from the panes
// below.
ToolBar {
    id: root

    property var library
    property var viewTags: []
    property var categories: []
    property bool fileView: true

    signal askInit
    signal askAddViewTag

    // Typing breaks the text: binding below, so re-sync the box when
    // navigation clears filter_query on the Rust side.
    Connections {
        target: root.library
        function onFilterQueryChanged() {
            if (searchField.text !== root.library.filter_query)
                searchField.text = root.library.filter_query;
        }
    }

    function baseName(path) {
        var parts = String(path || "").split("/").filter(function (p) {
            return p.length > 0;
        });
        return parts.length > 0 ? parts[parts.length - 1] : String(path || "");
    }
    function addableTags() {
        return root.categories.filter(function (c) {
            return root.viewTags.indexOf(c) === -1;
        });
    }
    function withoutTag(index) {
        return root.viewTags.filter(function (_, i) {
            return i !== index;
        });
    }

    topPadding: Spacing.xs
    bottomPadding: Spacing.xs

    background: Item {
        Rectangle {
            anchors.fill: parent
            color: AppTheme.surface
        }
        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: AppTheme.border
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Spacing.sm
        anchors.rightMargin: Spacing.sm
        spacing: Spacing.xs

        IconButton {
            glyph: library.is_repo ? Icons.refresh : Icons.plus
            enabled: !library.busy
            onClicked: {
                if (library.is_repo)
                    library.run_update();
                else
                    root.askInit();
            }
        }
        IconButton {
            glyph: Icons.up
            enabled: !library.busy
            onClicked: library.go_up()
        }
        Label {
            visible: !library.is_repo
            Layout.fillWidth: true
            text: library.cwd
            color: AppTheme.text
            elide: Text.ElideMiddle
            font: Typography.font(Typography.sm)
        }
        Label {
            visible: library.is_repo
            text: "/" + root.baseName(library.root)
            color: AppTheme.text
            font: Typography.font(Typography.sm)
        }
        IconButton {
            visible: library.is_repo
            glyph: root.fileView ? Icons.file : Icons.folder
            enabled: !library.busy
            onClicked: library.show_file_view(!root.fileView)
        }
        RowLayout {
            visible: library.is_repo && !root.fileView
            spacing: Spacing.xxs
            Repeater {
                model: root.viewTags
                delegate: RowLayout {
                    required property var modelData
                    required property int index
                    spacing: Spacing.xxs
                    Rectangle {
                        radius: 8
                        color: AppTheme.surface
                        border.color: AppTheme.border
                        border.width: 1
                        implicitWidth: Math.max(pillRow.implicitWidth + Spacing.sm, Base.rem(6))
                        implicitHeight: Base.rem(2)
                        RowLayout {
                            id: pillRow
                            anchors.fill: parent
                            anchors.leftMargin: Spacing.xs
                            anchors.rightMargin: Spacing.xs
                            spacing: Spacing.xxs
                            Label {
                                Layout.fillWidth: true
                                text: modelData
                                color: AppTheme.text
                                elide: Text.ElideRight
                                font: Typography.font(Typography.sm)
                            }
                            Icon {
                                glyph: Icons.trash
                                size: Typography.sm
                                color: AppTheme.muted
                            }
                        }
                        MouseArea {
                            anchors.fill: parent
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.library.set_view_tags(JSON.stringify(root.withoutTag(index)))
                        }
                    }
                    Icon {
                        visible: index < root.viewTags.length - 1 || root.addableTags().length > 0
                        glyph: Icons.chevronRight
                        size: Typography.sm
                        color: AppTheme.muted
                    }
                }
            }
            Rectangle {
                visible: root.addableTags().length > 0
                radius: 8
                color: AppTheme.surface
                border.color: AppTheme.border
                border.width: 1
                implicitWidth: Base.rem(2)
                implicitHeight: Base.rem(2)
                Icon {
                    anchors.centerIn: parent
                    glyph: Icons.plus
                    size: Typography.sm
                    color: AppTheme.text
                }
                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.askAddViewTag()
                }
            }
        }
        Label {
            visible: library.is_repo && !root.fileView && root.viewTags.length === 0 && root.addableTags().length === 0
            text: qsTr("(empty view)")
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
        Item {
            visible: library.is_repo
            Layout.fillWidth: true
        }
        TextField {
            id: searchField
            Layout.preferredWidth: Base.rem(12)
            placeholderText: library.is_repo ? qsTr("Search names & tags…") : qsTr("Search…")
            text: library.filter_query
            enabled: !library.busy
            onTextChanged: {
                if (text !== library.filter_query)
                    library.set_filter(text);
            }
        }
        IconButton {
            glyph: library.untagged_only ? Icons.alert : Icons.search
            glyphColor: library.untagged_only ? AppTheme.accent : AppTheme.text
            enabled: !library.busy
            onClicked: {
                if (searchField.text !== "") {
                    searchField.text = "";
                    library.set_filter("");
                } else {
                    library.toggle_untagged();
                }
            }
        }
        IconButton {
            glyph: Icons.external
            enabled: !library.busy
            onClicked: library.open_in_explorer()
        }
    }
}
