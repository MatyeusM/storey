import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.storey.gui

// Destructive-action confirm: message plus Confirm / No buttons.
// Confirm is preselected (default button + initial focus).
// Note: explicit width + contentItem layout — implicit sizes on
// direct Dialog children break scene loading.
Dialog {
    id: root

    property var action

    title: qsTr("Confirm")
    modal: true
    standardButtons: Dialog.Ok | Dialog.Cancel
    width: Base.rem(22)
    x: Math.round((parent.width - width) / 2)
    y: Math.round((parent.height - height) / 2)
    contentItem: ColumnLayout {
        Label {
            id: body
            Layout.fillWidth: true
            color: AppTheme.text
            wrapMode: Text.Wrap
        }
    }

    function ask(text, fn) {
        body.text = text;
        root.action = fn;
        root.open();
    }

    onOpened: {
        var ok = standardButton(Dialog.Ok);
        if (ok) {
            ok.text = qsTr("Confirm");
            ok.highlighted = true;
            ok.forceActiveFocus();
        }
        var no = standardButton(Dialog.Cancel);
        if (no)
            no.text = qsTr("No");
    }
    onAccepted: {
        if (root.action)
            root.action();
        root.action = null;
    }
    onRejected: {
        root.action = null;
    }
}
