import QtQuick
import com.storey.gui

// Win11 button: surface fill, hairline border, accent underline
// when checked. Pointer cursor throughout.
Item {
    id: root

    property string label: ""
    property bool checked: false

    signal clicked

    implicitWidth: labelItem.implicitWidth + Spacing.sm * 2
    implicitHeight: labelItem.implicitHeight + Spacing.xs * 2
    opacity: root.enabled ? 1 : 0.4

    Rectangle {
        anchors.fill: parent
        radius: 4
        color: hoverArea.containsMouse && root.enabled ? AppTheme.border : AppTheme.surface
        border.color: AppTheme.border
        border.width: 1
    }
    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: root.checked ? 2 : 0
        radius: 1
        color: AppTheme.accent
    }
    Text {
        id: labelItem
        anchors.centerIn: parent
        text: root.label
        color: AppTheme.text
        font: Typography.font(Typography.sm)
    }
    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
    }
}
