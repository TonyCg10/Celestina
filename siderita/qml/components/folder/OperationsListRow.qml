pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── One job, as a row ─────────────────────────────────────────────
    // What the expanded dock shows instead of a ring: the same arc at reading
    // size, the job's own name beside it, and its two controls on the right.
    // A ring plus a callout is the right shape for one job a person went
    // looking at; four of them is a list, and a list says which is which
    // without being asked.
Item {
    id: row

    required property string jobId
    required property string label
    required property string iconName
    required property int percent
    required property int steps
    required property bool paused

    signal pauseRequested(string id)
    signal cancelRequested(string id)

    implicitHeight: CelestinaTheme.controlHeightSm
    implicitWidth: ring.width + CelestinaTheme.spaceSm + name.implicitWidth
                   + CelestinaTheme.spaceMd + controls.width

    OperationRing {
        id: ring
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 26
        height: 26
        iconName: row.iconName
        percent: row.percent
        steps: row.steps
        paused: row.paused
        // In the list the ring is a read-out, not a button: the row's own
        // controls are right there, so there is nothing for a press to open.
        enabled: false
        activeFocusOnTab: false
        Accessible.name: row.label
    }

    Text {
        id: name
        anchors.left: ring.right
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.right: controls.left
        anchors.rightMargin: CelestinaTheme.spaceMd
        anchors.verticalCenter: parent.verticalCenter
        text: row.label
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        elide: Text.ElideRight
    }

    Row {
        id: controls
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: 2

        CelestinaIconButton {
            width: 26
            height: 26
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: row.paused ? "media-play" : "media-pause"
            helpText: row.paused ? qsTr("Reanudar") : qsTr("Pausar")
            onClicked: row.pauseRequested(row.jobId)
        }

        CelestinaIconButton {
            width: 26
            height: 26
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: "x"
            helpText: qsTr("Cancelar")
            onClicked: row.cancelRequested(row.jobId)
        }
    }
}
