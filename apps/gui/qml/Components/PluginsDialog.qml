import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Plugin health dialog: enabled checkboxes plus git-url install.
// Note: explicit width + contentItem layout — implicit sizes on
// direct Dialog children break scene loading.
Dialog {
    id: root

    property var library
    property var plugins: []

    signal askAddPlugin(string url)

    title: qsTr("Plugins")
    modal: true
    standardButtons: Dialog.Close
    width: Base.rem(30)
    height: Base.rem(24)
    x: Math.round((parent.width - width) / 2)
    y: Math.round((parent.height - height) / 2)
    contentItem: ColumnLayout {
        spacing: Spacing.xs

        RowLayout {
            Layout.fillWidth: true
            spacing: Spacing.xs
            TextField {
                id: urlField
                Layout.fillWidth: true
                placeholderText: qsTr("git url…")
            }
            PrimaryButton {
                label: qsTr("Add")
                onClicked: {
                    root.askAddPlugin(urlField.text);
                    urlField.text = "";
                }
            }
        }
        Label {
            visible: root.plugins.length === 0
            text: qsTr("no plugins yet")
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            model: root.plugins
            clip: true
            delegate: ItemDelegate {
                required property var modelData
                required property int index
                width: ListView.view.width
                background: Rectangle {
                    radius: 4
                    color: highlighted ? AppTheme.border : "transparent"
                }
                contentItem: RowLayout {
                    spacing: Spacing.xs
                    CheckBox {
                        checked: modelData.enabled
                        onToggled: root.library.set_plugin_enabled(index, checked)
                    }
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 0
                        Label {
                            Layout.fillWidth: true
                            text: modelData.name + " " + modelData.version
                            color: modelData.ok ? AppTheme.text : AppTheme.danger
                            elide: Text.ElideRight
                            font: Typography.font(Typography.sm)
                        }
                        Label {
                            Layout.fillWidth: true
                            text: modelData.path + " [" + modelData.status + "]"
                            color: AppTheme.muted
                            elide: Text.ElideMiddle
                            font: Typography.font(Typography.sm)
                        }
                    }
                }
            }
        }
    }

    onOpened: root.library.reload_plugins()
}
