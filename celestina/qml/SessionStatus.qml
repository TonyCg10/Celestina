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
        id: radio

        // Four states arrive here and only one of them is silence. An
        // unreadable adapter withdraws the provider, which is the absence this
        // guard sees; a machine with no controller says so once and then has
        // nothing to report either. What is left — on, with or without
        // anything on it — stays on the panel, because a powered radio is a
        // state a person needs to be able to see.
        readonly property string adapter: root.bluetooth !== undefined
                                          && root.bluetooth.adapter !== undefined
                                          ? root.bluetooth.adapter : ""
        readonly property int count: root.bluetooth !== undefined
                                     && root.bluetooth.count !== undefined
                                     ? root.bluetooth.count : 0

        visible: adapter === "on" || adapter === "off"
        anchors.verticalCenter: parent.verticalCenter
        text: {
            if (radio.adapter === "off")
                return qsTr("bt apagado");

            if (radio.adapter !== "on")
                return "";

            // The count is the news when there is one; a powered radio with
            // nothing on it says only that it is on.
            return radio.count > 0 ? qsTr("bt %1").arg(radio.count) : qsTr("bt");
        }
        // A radio nothing is using is quieter than one carrying a device.
        color: radio.adapter === "on" && radio.count > 0
               ? CelestinaTheme.text : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.features: CelestinaTheme.fontFeaturesTabular
        font.pixelSize: CelestinaTheme.fontCaption
        Accessible.role: Accessible.StaticText
        Accessible.name: {
            if (radio.adapter === "off")
                return qsTr("Bluetooth apagado");

            if (radio.adapter !== "on")
                return "";

            if (radio.count === 0)
                return qsTr("Bluetooth encendido, sin dispositivos conectados");

            return root.bluetooth.first !== undefined
                   ? qsTr("Bluetooth: %1 conectado, %2 en total").arg(root.bluetooth.first).arg(radio.count)
                   : qsTr("Bluetooth: %1 conectados").arg(radio.count);
        }
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
