// How the session is connected.
//
// Network and Bluetooth share one compact row and open their own menus. Power
// profiles already have their complete state and action in ControlCentre, so
// the panel does not duplicate that control here.
//
// The network entry stays on the bar whenever the provider is publishing
// anything at all, not only when a link is confirmed. A session with no default
// route is exactly when somebody needs the menu that lists the networks it
// could join, and keying visibility off `network.kind` took that entry point
// away at the moment it mattered.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Row {
    id: root

    // The `network` and `bluetooth` providers, or `undefined` when a provider
    // has nothing to publish. `var` is necessary: QML has no typed map.
    required property var network
    required property var bluetooth
    required property BackdropInk ink
    // Each indicator asks for its own menu at the point it was clicked. The
    // panel forwards it; the host owns every surface this row does not.
    signal indicatorMenuRequested(string kind, int globalX, int globalY,
                                  int openerWidth, int openerHeight)

    readonly property bool hasVisibleIndicator: link.visible || radio.visible

    // PANEL-1 — a Row aligns its children at the top, so readings of different
    // heights sat at different heights. One height each.
    height: CelestinaTheme.controlHeightXs
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
    component Indicator: PanelMenuButton {
        id: control

        required property string kind
        required property string reading
        required property string iconName

        ink: root.ink

        anchors.verticalCenter: parent ? parent.verticalCenter : undefined
        implicitWidth: CelestinaTheme.iconSm
        implicitHeight: parent ? parent.height : CelestinaTheme.iconSm
        text: control.reading
        Accessible.role: Accessible.Button
        // `PanelMenuButton` owns pressed, hover, focus and opener geometry;
        // this row contributes only which menu that rectangle names.
        onMenuRequested: (globalX, globalY, openerWidth, openerHeight) =>
            root.indicatorMenuRequested(control.kind, globalX, globalY,
                                        openerWidth, openerHeight)

        contentItem: Item {
            implicitWidth: glyph.width
            implicitHeight: glyph.height

            CelestinaIcon {
                id: glyph

                anchors.centerIn: parent
                width: CelestinaTheme.iconSm
                height: CelestinaTheme.iconSm
                name: control.iconName
                tone: CelestinaIcon.Primary
                tintOverride: control.ink.primary
                Accessible.ignored: true
            }
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
        iconName: "wifi"
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
        Accessible.onPressAction: link.requestMenu()
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
        iconName: "bluetooth"
        reading: {
            if (radio.adapter === "off")
                return qsTr("bt apagado");

            if (radio.adapter !== "on")
                return "";

            // The count is the news when there is one; a powered radio with
            // nothing on it says only that it is on.
            return radio.count > 0 ? qsTr("bt %1").arg(radio.count) : qsTr("bt");
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
        Accessible.onPressAction: radio.requestMenu()
    }

}
