import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Add one tag category to the materialized view. Only categories
// present in the library can be picked: Add refuses anything
// outside the list instead of closing.
// Note: explicit width + contentItem layout — implicit sizes on
// direct Dialog children break scene loading.
Dialog {
    id: root

    property var library
    property var viewTags: []
    property var categories: []

    title: qsTr("Add to view")
    modal: true
    width: Base.rem(24)
    x: Math.round((parent.width - width) / 2)
    y: Math.round((parent.height - height) / 2)
    contentItem: ColumnLayout {
        spacing: Spacing.xs

        Label {
            Layout.fillWidth: true
            text: qsTr("Tag category")
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
        ComboBox {
            id: catBox
            Layout.fillWidth: true
            editable: true
            model: root.options()
            enabled: !library.busy
        }
        Label {
            id: errLabel
            visible: text !== ""
            Layout.fillWidth: true
            color: AppTheme.danger
            font: Typography.font(Typography.sm)
            wrapMode: Text.Wrap
        }
        RowLayout {
            spacing: Spacing.xs
            PrimaryButton {
                label: qsTr("Add")
                enabled: !library.busy
                onClicked: {
                    var pick = catBox.editText.trim();
                    if (root.options().indexOf(pick) === -1) {
                        errLabel.text = qsTr("Pick a tag from the list.");
                        return;
                    }
                    root.library.set_view_tags(JSON.stringify(root.viewTags.concat([pick])));
                    root.close();
                }
            }
            PrimaryButton {
                label: qsTr("Cancel")
                onClicked: root.close()
            }
        }
    }

    function options() {
        return root.categories.filter(function (c) {
            return root.viewTags.indexOf(c) === -1;
        });
    }
    function openWith() {
        errLabel.text = "";
        catBox.currentIndex = 0;
        root.open();
    }
}
