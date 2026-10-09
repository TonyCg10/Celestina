pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.magnetita 1.0

// The chooser `magnetita --send` opens when several devices are connected:
// the files are already chosen, the person picks where they go, and the
// window closes. The names are the devices the launch listed, in its order,
// so a pick is an index into that list.
Column {
    id: root

    required property var deviceNames
    required property int fileCount
    // While the files go, and the failure when they did not: the window
    // stays up to say so instead of closing on a send that never arrived.
    property bool busy: false
    property string failure: ""
    signal chosen(int index)
    signal cancelled

    spacing: CelestinaTheme.spaceMd

    Text {
        width: parent.width
        text: qsTr("Enviar %n archivo(s) a…", "", root.fileCount)
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontTitle
        font.weight: CelestinaTheme.weightDemiBold
        wrapMode: Text.Wrap
    }

    Repeater {
        model: root.deviceNames

        delegate: CelestinaButton {
            id: deviceButton
            required property string modelData
            required property int index

            width: root.width
            density: CelestinaButton.Regular
            // Peer-supplied text: the button paints it as plain text.
            text: deviceButton.modelData
            helpText: qsTr("Enviar a %1").arg(deviceButton.modelData)
            enabled: !root.busy && root.failure.length === 0
            onClicked: root.chosen(deviceButton.index)
        }
    }

    Text {
        visible: root.busy || root.failure.length > 0
        width: parent.width
        // The daemon's message is development text; the line before it is
        // what the person reads.
        textFormat: Text.PlainText
        text: root.busy ? qsTr("Enviando…")
                        : qsTr("No se pudo enviar: %1").arg(root.failure)
        color: root.busy ? CelestinaTheme.textMuted : CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        wrapMode: Text.Wrap
    }

    CelestinaButton {
        width: root.width
        role: CelestinaButton.Ghost
        enabled: !root.busy
        text: root.failure.length > 0 ? qsTr("Cerrar") : qsTr("Cancelar")
        onClicked: root.cancelled()
    }
}
