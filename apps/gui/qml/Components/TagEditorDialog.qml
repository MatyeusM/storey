import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Tag editor for one file: set/add tags plus plugin suggestions.
// Note: explicit width + contentItem layout — implicit sizes on
// direct Dialog children break scene loading.
Dialog {
    id: root

    property var library
    property var detail
    property var categories: []
    property var suggestions: []

    title: qsTr("Tags")
    modal: true
    standardButtons: Dialog.Close
    width: Base.rem(28)
    height: Base.rem(26)
    x: Math.round((parent.width - width) / 2)
    y: Math.round((parent.height - height) / 2)
    contentItem: ColumnLayout {
        spacing: Spacing.xs

        Label {
            Layout.fillWidth: true
            text: root.detail ? root.detail.displayName : qsTr("No selection")
            color: AppTheme.text
            font: Typography.font(Typography.md)
            elide: Text.ElideRight
        }
        Label {
            visible: root.detail !== null
            text: root.detail ? ("id: " + root.detail.shortId + "  size: " + root.detail.size) : ""
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
        }
        Label {
            Layout.fillWidth: true
            visible: root.detail !== null
            text: root.detail ? ("tags: " + root.detail.tagsText) : ""
            color: AppTheme.muted
            font: Typography.font(Typography.sm)
            wrapMode: Text.Wrap
        }
        ComboBox {
            id: catBox
            Layout.fillWidth: true
            editable: true
            model: root.categories
            enabled: root.detail !== null && !library.busy
            onEditTextChanged: root.refreshPrefill()
        }
        TextField {
            id: valsField
            Layout.fillWidth: true
            placeholderText: qsTr("values, comma-separated")
            enabled: root.detail !== null && !library.busy
        }
        RowLayout {
            PrimaryButton {
                label: qsTr("Apply tags")
                enabled: root.detail !== null && !library.busy
                onClicked: library.apply_tags(catBox.editText, valsField.text)
            }
        }
        RowLayout {
            PrimaryButton {
                label: qsTr("Suggest")
                enabled: root.detail !== null && !library.busy
                onClicked: library.do_suggest()
            }
            PrimaryButton {
                label: qsTr("Apply all")
                visible: root.suggestions.length > 0
                enabled: !library.busy
                onClicked: library.apply_suggestions()
            }
        }
        ListView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            visible: root.suggestions.length > 0
            model: root.suggestions
            clip: true
            delegate: Label {
                required property var modelData
                width: ListView.view.width
                text: modelData.category + "=" + modelData.value + "  (" + Number(modelData.score).toFixed(2) + " " + modelData.source + ")"
                color: AppTheme.muted
                font: Typography.font(Typography.sm)
            }
        }
    }

    function refreshPrefill() {
        var vals = root.detail && root.detail.tags ? root.detail.tags[catBox.editText] : undefined;
        valsField.text = vals ? vals.join(", ") : "";
    }

    onOpened: root.refreshPrefill()
}
