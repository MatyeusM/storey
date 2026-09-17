import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Status footer under the file grid: storey state on the left,
// folder/file/size counts on the right. Quiet when all is normal.
ToolBar {
    id: root

    property var library

    // The "use file explorer" hint is transient: decay it after 10s.
    // (Matches nav.rs blocked_message; real errors stay put.)
    Timer {
        id: decayTimer
        interval: 10000
        onTriggered: root.library.dismiss_status()
    }
    Connections {
        target: root.library
        function onStatusTextChanged() {
            if (root.library.status_text.indexOf("Use file explorer") !== -1)
                decayTimer.restart();
            else
                decayTimer.stop();
        }
    }

    position: ToolBar.Footer
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
            anchors.top: parent.top
            height: 1
            color: AppTheme.border
        }
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: Spacing.sm
        anchors.rightMargin: Spacing.sm
        spacing: Spacing.sm

        Label {
            Layout.fillWidth: true
            text: {
                if (library.busy && library.progress_label !== "")
                    return library.progress_label;
                return library.status_text;
            }
            color: library.status_text.indexOf("storey error") === 0 ? AppTheme.danger : AppTheme.muted
            elide: Text.ElideRight
            font: Typography.font(Typography.sm)
        }
        ProgressBar {
            visible: library.busy
            indeterminate: library.progress_fraction < 0
            value: library.progress_fraction < 0 ? 0 : library.progress_fraction
            Layout.preferredWidth: Base.rem(8)
        }
        Label {
            text: qsTr("%1 folders").arg(library.folder_count)
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
        Label {
            text: qsTr("%1 files · %2").arg(library.file_count).arg(library.size_text)
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
    }
}
