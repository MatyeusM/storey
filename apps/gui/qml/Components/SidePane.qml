import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Left quick-access pane: XDG places plus the plugins dialog entry.
// The right hairline separates it from the file grid.
Frame {
    id: root

    property var library
    property var places: []

    signal openPlace(string path)
    signal openPlugins

    topPadding: Spacing.sm
    bottomPadding: Spacing.sm
    leftPadding: Spacing.xs
    rightPadding: Spacing.xs

    background: Item {
        Rectangle {
            anchors.fill: parent
            color: AppTheme.surface
        }
        Rectangle {
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.right: parent.right
            width: 1
            color: AppTheme.border
        }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: Spacing.xxs

        Label {
            text: qsTr("Quick access")
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
            Layout.leftMargin: Spacing.xs
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            model: root.places
            clip: true
            delegate: ItemDelegate {
                required property var modelData
                width: ListView.view.width
                highlighted: library.cwd === modelData.path
                background: Rectangle {
                    radius: 4
                    color: highlighted ? AppTheme.border : "transparent"
                }
                contentItem: RowLayout {
                    spacing: Spacing.xs
                    Icon {
                        glyph: modelData.name === "Home" ? Icons.home : Icons.folder
                        size: Typography.sm
                        color: AppTheme.accent
                    }
                    Label {
                        Layout.fillWidth: true
                        text: modelData.name
                        color: AppTheme.text
                        elide: Text.ElideRight
                        font: Typography.font(Typography.sm)
                    }
                }
                onClicked: root.openPlace(modelData.path)
            }
        }
        PrimaryButton {
            Layout.fillWidth: true
            label: qsTr("Plugins")
            onClicked: root.openPlugins()
        }
    }
}
