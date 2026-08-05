// Ending the session, or the machine's day.
//
// Four requests a person cannot take back, so each one is asked twice: the
// first press arms it and says what it will do, the second sends it. Nothing
// here is a hover away from happening, and Escape disarms.
//
// The outcome is shown rather than assumed. Reboot and power off are asked of
// the session manager, which may refuse — an inhibitor, another session, no
// permission — and that refusal is the useful thing to see. Suspend is refused
// by this shell itself while no locker exists, and says so in the same place.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: menu

    required property var shellSource
    required property bool reducedMotion

    signal dismissed()

    readonly property int cardWidth: 360

    // `verb` is the session channel's own vocabulary; nothing here invents a
    // name for an action.
    readonly property var actions: [
        {"verb": "log-out", "label": qsTr("Log out"),
         "warning": qsTr("This ends the session and closes everything open.")},
        {"verb": "reboot", "label": qsTr("Restart"),
         "warning": qsTr("This restarts the machine.")},
        {"verb": "power-off", "label": qsTr("Power off"),
         "warning": qsTr("This shuts the machine down.")},
        {"verb": "suspend", "label": qsTr("Suspend"),
         "warning": qsTr("This sleeps the machine.")}
    ]

    property string armed: ""
    property string outcomeVerb: ""
    property string outcomeState: ""
    property string outcomeReason: ""

    width: cardWidth
    height: column.implicitHeight + CelestinaTheme.spaceLg * 2
    color: CelestinaTheme.clear
    title: qsTr("Session")

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = menu.reducedMotion;
        column.forceActiveFocus();
    }

    function press(verb) {
        if (menu.armed !== verb) {
            // First press arms it and nothing else happens.
            menu.armed = verb;
            return;
        }
        menu.armed = "";
        menu.outcomeVerb = verb;
        menu.outcomeState = "pending";
        menu.outcomeReason = "";
        if (menu.shellSource)
            menu.shellSource.send(verb);
    }

    Connections {
        function onCommandOutcome(verb, state, reason) {
            if (verb !== menu.outcomeVerb)
                return;
            menu.outcomeState = state;
            menu.outcomeReason = reason;
        }

        target: menu.shellSource
    }

    Item {
        id: scene

        anchors.fill: parent

        GlassCard {
            anchors.fill: parent
            backdropSource: scene
            Accessible.role: Accessible.Dialog
            Accessible.name: qsTr("Session")

            Column {
                id: column

                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceXs
                focus: true

                Keys.onEscapePressed: {
                    // Escape disarms before it dismisses: leaving an armed
                    // action behind would be leaving a loaded control.
                    if (menu.armed.length > 0) {
                        menu.armed = "";
                        return;
                    }
                    menu.dismissed();
                }

                Repeater {
                    model: menu.actions

                    delegate: Column {
                        id: entry

                        required property var modelData

                        readonly property bool isArmed: menu.armed === entry.modelData.verb

                        width: column.width
                        spacing: 1

                        CelestinaButton {
                            width: entry.width
                            text: entry.isArmed
                                  ? qsTr("%1 — press again").arg(entry.modelData.label)
                                  : entry.modelData.label
                            role: entry.isArmed ? CelestinaButton.Destructive
                                                : CelestinaButton.Tonal
                            Accessible.name: entry.isArmed
                                    ? qsTr("%1. %2 Press again to confirm.")
                                      .arg(entry.modelData.label)
                                      .arg(entry.modelData.warning)
                                    : entry.modelData.label
                            onClicked: menu.press(entry.modelData.verb)
                        }

                        Text {
                            width: entry.width
                            visible: entry.isArmed
                                     || menu.outcomeVerb === entry.modelData.verb
                            text: {
                                if (entry.isArmed)
                                    return entry.modelData.warning;
                                if (menu.outcomeState === "pending")
                                    return qsTr("asking…");
                                if (menu.outcomeState === "failed") {
                                    return menu.outcomeReason.length > 0
                                           ? qsTr("refused: %1").arg(menu.outcomeReason)
                                           : qsTr("refused");
                                }
                                return qsTr("the session manager accepted this");
                            }
                            color: menu.outcomeState === "failed"
                                   && menu.outcomeVerb === entry.modelData.verb
                                   ? CelestinaTheme.danger : CelestinaTheme.textMuted
                            wrapMode: Text.WordWrap
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                            bottomPadding: CelestinaTheme.spaceXs
                        }
                    }
                }
            }
        }
    }
}
