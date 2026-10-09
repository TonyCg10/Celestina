import QtQuick
import org.celestina.calcita 1.0

// The confirmation an external link needs before it leaves Calcita: «Abrir
// enlace externo» with the link's host, «Abrir» and «Cancelar». Nothing is
// opened until the person says so; Escape cancels.
Item {
    id: pill

    // The link waiting for an answer, empty when none is.
    property string url: ""
    readonly property bool asking: pill.url.length > 0
    // What the pill names: the host of a web link, the whole link otherwise.
    readonly property string host: pill.hostOf(pill.url)

    signal confirmed(string url)

    function ask(url) {
        pill.url = url
        openButton.forceActiveFocus()
    }

    function cancel() {
        pill.url = ""
    }

    function hostOf(url) {
        const match = /^[a-z][a-z0-9+.-]*:\/\/([^/?#:]+)/i.exec(url)
        return match !== null ? match[1] : url
    }

    objectName: "linkPill"
    visible: opacity > 0
    opacity: pill.asking ? 1 : 0
    implicitWidth: row.implicitWidth + CelestinaTheme.spaceLg * 2
    implicitHeight: row.implicitHeight + CelestinaTheme.spaceSm * 2
    Accessible.role: Accessible.AlertMessage
    Accessible.name: qsTr("Abrir enlace externo") + " " + pill.host

    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionNormal
        }
    }

    Keys.onEscapePressed: pill.cancel()

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Elevated
        radiusOverride: CelestinaTheme.radiusPill
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceSm

        Column {
            anchors.verticalCenter: parent.verticalCenter

            Text {
                text: qsTr("Abrir enlace externo")
                color: CelestinaTheme.text
                font.pixelSize: CelestinaTheme.fontBody
                font.weight: CelestinaTheme.weightDemiBold
            }

            Text {
                objectName: "linkHost"
                width: Math.min(implicitWidth, 280)
                elide: Text.ElideMiddle
                text: pill.host
                color: CelestinaTheme.textMuted
                font.pixelSize: CelestinaTheme.fontCaption
            }
        }

        CelestinaButton {
            objectName: "cancelLinkButton"
            anchors.verticalCenter: parent.verticalCenter
            role: CelestinaButton.Ghost
            text: qsTr("Cancelar")
            onClicked: pill.cancel()
        }

        CelestinaButton {
            id: openButton
            objectName: "openLinkButton"
            anchors.verticalCenter: parent.verticalCenter
            role: CelestinaButton.Primary
            text: qsTr("Abrir")
            onClicked: {
                const url = pill.url
                pill.cancel()
                pill.confirmed(url)
            }
        }
    }
}
