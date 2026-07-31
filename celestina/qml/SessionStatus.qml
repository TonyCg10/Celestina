// How the session is online, what is connected to it, and how it is running.
//
// Three read-only readings sharing one row, each present only when it has
// something to say. Controls for the first two arrive with R5's control
// centre; the power profile is the one that can be asked to change here, and
// even that is a request — the panel paints what the daemon reports next, never
// the profile it asked for.
import CelestinaStyle
import QtQuick

Row {
    id: root

    // The `network`, `bluetooth` and `power` providers, or `undefined` when a
    // provider has nothing to publish. `var` is necessary: QML has no typed map.
    required property var network
    required property var bluetooth
    required property var power
    signal profileCycleRequested()

    readonly property var profileNames: ({
        "performance": qsTr("rendimiento"),
        "balanced": qsTr("equilibrado"),
        "power-saver": qsTr("ahorro")
    })

    spacing: CelestinaTheme.spaceMd

    Text {
        id: link

        readonly property bool present: root.network !== undefined
                                        && root.network.kind !== undefined

        visible: present
        anchors.verticalCenter: parent.verticalCenter
        // The kind of link is what a glance needs; its name is what tells two
        // networks apart, so the accessible name carries both.
        text: {
            if (!present)
                return "";

            return root.network.kind === "ethernet" ? qsTr("cable") : root.network.connection;
        }
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        elide: Text.ElideRight
        Accessible.role: Accessible.StaticText
        Accessible.name: present
                         ? qsTr("Conectado por %1 a %2").arg(root.network.kind).arg(root.network.connection)
                         : ""
    }

    Text {
        readonly property bool present: root.bluetooth !== undefined
                                        && root.bluetooth.count !== undefined

        visible: present
        anchors.verticalCenter: parent.verticalCenter
        // Only ever shown when something is connected, so the count is the
        // news and the name is the detail.
        text: present ? qsTr("bt %1").arg(root.bluetooth.count) : ""
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.features: CelestinaTheme.fontFeaturesTabular
        font.pixelSize: CelestinaTheme.fontCaption
        Accessible.role: Accessible.StaticText
        Accessible.name: present
                         ? qsTr("Bluetooth: %1 conectado").arg(root.bluetooth.first)
                         : ""
    }

    Text {
        id: profile

        readonly property bool present: root.power !== undefined
                                        && root.power.active !== undefined
        // A daemon offering one profile has nothing to cycle through.
        readonly property bool cyclable: present && root.power.count > 1

        visible: present
        anchors.verticalCenter: parent.verticalCenter
        text: present
              ? (root.profileNames[root.power.active] !== undefined
                 ? root.profileNames[root.power.active] : root.power.active)
              : ""
        color: present && root.power.active === "performance"
               ? CelestinaTheme.accentLink : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        Accessible.role: cyclable ? Accessible.Button : Accessible.StaticText
        Accessible.name: present ? qsTr("Perfil de energía: %1").arg(text) : ""
        Accessible.description: cyclable ? qsTr("Cambia al siguiente perfil") : ""
        Accessible.onPressAction: root.profileCycleRequested()

        MouseArea {
            anchors.fill: parent
            enabled: profile.cyclable
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.profileCycleRequested()
        }

    }

}
