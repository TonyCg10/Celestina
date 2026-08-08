// How the session is online, what is connected to it, and how it is running.
//
// Three readings sharing one row. The first two open their own menus; the power
// profile is a request like any other — the panel paints what the daemon
// reports next, never the profile it asked for.
//
// The network entry stays on the bar whenever the provider is publishing
// anything at all, not only when a link is confirmed. A session with no default
// route is exactly when somebody needs the menu that lists the networks it
// could join, and keying visibility off `network.kind` took that entry point
// away at the moment it mattered.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Controls

Row {
    id: root

    // The `network`, `bluetooth` and `power` providers, or `undefined` when a
    // provider has nothing to publish. `var` is necessary: QML has no typed map.
    required property var network
    required property var bluetooth
    required property var power
    signal profileCycleRequested()
    // Each indicator asks for its own menu at the point it was clicked. The
    // panel forwards it; the host owns every surface this row does not.
    signal indicatorMenuRequested(string kind, int globalX, int globalY)

    readonly property var profileNames: ({
        "performance": qsTr("rendimiento"),
        "balanced": qsTr("equilibrado"),
        "power-saver": qsTr("ahorro")
    })

    spacing: CelestinaTheme.spaceMd

    // A reading that can be opened.
    //
    // `AbstractButton` rather than a `Text` with a `MouseArea`, because a
    // reading that does something is a control: it takes focus, shows that it
    // has it through `visualFocus` — which is true for the keyboard and false
    // for a click, exactly as it should be — and answers Space and Enter.
    //
    // The panel's own surface refuses the keyboard by design (`panelSpec` maps
    // it `KeyboardInteractivityNone`), so on a live session nothing tabs here.
    // That is the panel's decision, not this control's: wherever a surface does
    // admit the keyboard, this works, and the offscreen cases drive it there.
    component Indicator: AbstractButton {
        id: control

        required property string kind
        required property string reading

        function askForMenu() {
            const at = control.mapToGlobal(0, control.height);
            root.indicatorMenuRequested(control.kind, at.x, at.y);
        }

        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        implicitWidth: label.implicitWidth
        implicitHeight: label.implicitHeight
        padding: 0
        activeFocusOnTab: true
        text: control.reading
        Accessible.role: Accessible.Button
        // `AbstractButton` already reports `Accessible.pressed`; the name,
        // description and action are this control's to say.
        onClicked: control.askForMenu()
        // Space is `AbstractButton`'s own, and produces `clicked`. Return and
        // Enter are not, so they are named here rather than left out.
        Keys.onReturnPressed: control.askForMenu()
        Keys.onEnterPressed: control.askForMenu()

        contentItem: Text {
            id: label

            text: control.reading
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }

        background: Item {
            CelestinaFocusRing {
                objectName: "celestina-indicator-focus"
                target: control
                cornerRadius: CelestinaTheme.radiusSm
                shown: control.visualFocus
            }

        }

        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.NoButton
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
        }

    }

    Indicator {
        id: link

        readonly property bool linked: root.network !== undefined
                                       && root.network.kind !== undefined
        // What the provider says about the saved-network list, or the empty
        // string when it says nothing. `pending`, `held` and `unavailable` are
        // all readings worth an entry point; only a provider that has withdrawn
        // entirely leaves nothing to open.
        readonly property string listState: root.network !== undefined
                                            && root.network.networksState !== undefined
                                            ? root.network.networksState : ""

        objectName: "celestina-network-indicator"
        // Present whenever the provider is publishing. A withdrawn provider is
        // an unreadable session, and there is nothing truthful to offer then.
        visible: root.network !== undefined
        kind: "network"
        // The kind of link is what a glance needs; its name is what tells two
        // networks apart, so the accessible name carries both. With no link the
        // entry says so plainly rather than naming a connection it does not
        // have — and never claims Wi-Fi merely because an inventory exists.
        reading: {
            if (!link.linked)
                return qsTr("sin red");

            return root.network.kind === "ethernet"
                   ? qsTr("cable") : root.network.connection;
        }
        Accessible.name: link.linked
                         ? qsTr("Conectado por %1 a %2").arg(root.network.kind).arg(root.network.connection)
                         : qsTr("Sin conexión de red")
        Accessible.description: qsTr("Abre el menú de red")
        Accessible.onPressAction: link.askForMenu()
    }

    Indicator {
        id: radio

        // Four states arrive here and only one of them is silence. An
        // unreadable adapter withdraws the provider, which is the absence this
        // guard sees; a machine with no controller says so once and then has
        // nothing to report either. What is left — on, with or without anything
        // on it — stays on the panel, because a powered radio is a state a
        // person needs to be able to see.
        readonly property string adapter: root.bluetooth !== undefined
                                          && root.bluetooth.adapter !== undefined
                                          ? root.bluetooth.adapter : ""
        readonly property int count: root.bluetooth !== undefined
                                     && root.bluetooth.count !== undefined
                                     ? root.bluetooth.count : 0

        objectName: "celestina-bluetooth-indicator"
        visible: adapter === "on" || adapter === "off"
        kind: "bluetooth"
        reading: {
            if (radio.adapter === "off")
                return qsTr("bt apagado");

            if (radio.adapter !== "on")
                return "";

            // The count is the news when there is one; a powered radio with
            // nothing on it says only that it is on.
            return radio.count > 0 ? qsTr("bt %1").arg(radio.count) : qsTr("bt");
        }
        // A radio nothing is using is quieter than one carrying a device.
        contentItem: Text {
            text: radio.reading
            color: radio.adapter === "on" && radio.count > 0
                   ? CelestinaTheme.text : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.features: CelestinaTheme.fontFeaturesTabular
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }
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
        Accessible.description: qsTr("Abre el menú de Bluetooth")
        Accessible.onPressAction: radio.askForMenu()
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
