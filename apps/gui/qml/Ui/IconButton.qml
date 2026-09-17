import QtQuick
import com.storey.gui

// Win11 icon button: transparent, subtle hover fill, rounded.
// Built on MouseArea because Controls have no cursorShape property.
Item {
    id: root

    property string glyph: ""
    property real size: Typography.lg
    property color glyphColor: AppTheme.text

    signal clicked

    implicitWidth: Base.rem(2)
    implicitHeight: Base.rem(2)
    opacity: root.enabled ? 1 : 0.4

    Rectangle {
        anchors.fill: parent
        radius: 4
        color: hoverArea.containsMouse && root.enabled ? AppTheme.border : "transparent"
    }
    Icon {
        anchors.centerIn: parent
        glyph: root.glyph
        size: root.size
        color: root.glyphColor
    }
    MouseArea {
        id: hoverArea
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: root.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: root.clicked()
    }
}
