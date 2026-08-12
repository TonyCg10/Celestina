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
    property alias anchoredFromPanel: placement.anchoredFromPanel
    property alias openerRect: placement.openerRect
    property alias attachmentAnchorRect: placement.attachmentAnchorRect
    property alias attachmentStartY: placement.attachmentStartY
    property alias compositorBlurAvailable: card.compositorBlurAvailable
    property alias glassRects: card.glassRects
    property alias glassRegions: card.glassRegions

    signal dismissed()

    BackdropInk {
        id: backdropInk
    }

    readonly property int cardWidth: 360
    // The card grows with its own content; naming it is what lets the surface
    // be the whole output while the card stays the size of what it says.
    readonly property int cardHeight: contentColumn.implicitHeight
                                      + CelestinaTheme.spaceMd * 2
    readonly property int anchorGap: placement.anchorGap
    readonly property real cardX: placement.x
    readonly property real cardY: placement.y

    // `verb` is the session channel's own vocabulary; nothing here invents a
    // name for an action.
    readonly property var actions: [
        {"verb": "log-out", "label": qsTr("Cerrar sesión"),
         "warning": qsTr("Esto cierra la sesión y todo lo que haya abierto.")},
        {"verb": "reboot", "label": qsTr("Reiniciar"),
         "warning": qsTr("Esto reinicia el equipo.")},
        {"verb": "power-off", "label": qsTr("Apagar"),
         "warning": qsTr("Esto apaga el equipo.")},
        {"verb": "suspend", "label": qsTr("Suspender"),
         "warning": qsTr("Esto suspende el equipo.")}
    ]

    property string armed: ""
    property string outcomeVerb: ""
    property string outcomeState: ""
    property string outcomeReason: ""

    color: CelestinaTheme.clear
    title: qsTr("Sesión")

    Component.onCompleted: {
        // These are bootstrap dimensions, not bindings. Once layer-shell gives
        // this Window the output size, confirmation or outcome copy may grow
        // the card without shrinking the input surface back around it.
        menu.width = menu.cardWidth;
        menu.height = menu.cardHeight;
        CelestinaTheme.reducedMotion = menu.reducedMotion;
        column.forceActiveFocus();
    }

    PanelPopupPlacement {
        id: placement

        surfaceWidth: menu.width
        surfaceHeight: menu.height
        contentWidth: menu.cardWidth
        contentHeight: menu.cardHeight
        edgeInset: 0
    }

    onVisibleChanged: {
        if (visible)
            Qt.callLater(card.reveal);
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

    function iconFor(verb) {
        switch (verb) {
        case "log-out":
            return "unplug";
        case "reboot":
            return "rotate-ccw";
        case "power-off":
            return "power";
        default:
            return "media-pause";
        }
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

    // A click anywhere outside the card closes this surface.
    //
    // The surface is the whole output, not the card: that is what makes an
    // outside click land here at all, and it is also what makes the panel
    // button that opened this close it in one click rather than two. While the
    // overlay is up the button is behind it, so the click never reaches the
    // panel, never re-enters `toggle()`, and focus returns exactly once.
    MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
        onPressed: menu.dismissed()
    }

    SoftOverlayCard {
        id: card

        ink: backdropInk
        width: menu.cardWidth
        height: menu.cardHeight
        x: menu.cardX
        y: menu.cardY
        reducedMotion: menu.reducedMotion
        accessibleName: qsTr("Sesión")
        attachedToTop: menu.anchoredFromPanel
        openerRect: menu.openerRect
        attachmentAnchorRect: menu.attachmentAnchorRect
        attachmentStartY: menu.attachmentStartY
        surfacePosition: Qt.point(menu.cardX, menu.cardY)

        Column {
            id: contentColumn

            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceMd
            spacing: CelestinaTheme.spaceSm

            MenuHeader {
                width: parent.width
                ink: backdropInk
                title: qsTr("Sesión")
                subtitle: qsTr("Acciones del sistema")
                iconName: "power"
            }

            Item {
                width: parent.width
                implicitHeight: column.implicitHeight + CelestinaTheme.spaceXs * 2

                MenuSection { ink: backdropInk }

                Column {
                    id: column

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceXs
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

                            readonly property bool isArmed: menu.armed
                                                              === entry.modelData.verb

                            width: column.width
                            spacing: 1

                            BackdropButton {
                                id: actionButton

                                width: entry.width
                                implicitHeight: CelestinaTheme.controlHeightLg
                                ink: backdropInk
                                text: entry.modelData.label
                                role: entry.isArmed ? CelestinaButton.Destructive
                                                    : CelestinaButton.Ghost
                                Accessible.name: entry.isArmed
                                        ? qsTr("%1. %2 Pulsa otra vez para confirmar.")
                                          .arg(entry.modelData.label)
                                          .arg(entry.modelData.warning)
                                        : entry.modelData.label
                                onClicked: menu.press(entry.modelData.verb)

                                contentItem: Item {
                                    CelestinaIcon {
                                        id: actionIcon

                                        anchors.left: parent.left
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: CelestinaTheme.iconSm
                                        height: width
                                        name: menu.iconFor(entry.modelData.verb)
                                        fallbackName: "power"
                                        tintOverride: entry.isArmed
                                                      ? CelestinaTheme.dangerFillInk
                                                      : backdropInk.primary
                                        Accessible.ignored: true
                                    }

                                    Text {
                                        anchors.left: actionIcon.right
                                        anchors.leftMargin: CelestinaTheme.spaceMd
                                        anchors.right: actionState.left
                                        anchors.rightMargin: CelestinaTheme.spaceSm
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: entry.isArmed
                                              ? qsTr("%1 — pulsa otra vez")
                                                .arg(entry.modelData.label)
                                              : entry.modelData.label
                                        textFormat: Text.PlainText
                                        color: entry.isArmed
                                               ? CelestinaTheme.dangerFillInk
                                               : backdropInk.primary
                                        font.family: CelestinaTheme.sansFamily
                                        font.pixelSize: CelestinaTheme.fontBody
                                        font.weight: CelestinaTheme.weightDemiBold
                                        elide: Text.ElideRight
                                    }

                                    CelestinaIcon {
                                        id: actionState

                                        anchors.right: parent.right
                                        anchors.verticalCenter: parent.verticalCenter
                                        width: CelestinaTheme.iconSm
                                        height: width
                                        name: entry.isArmed ? "circle-alert" : "go-next"
                                        fallbackName: "go-next"
                                        tintOverride: entry.isArmed
                                                      ? CelestinaTheme.dangerFillInk
                                                      : backdropInk.muted
                                        Accessible.ignored: true
                                    }
                                }
                            }

                            Text {
                                width: entry.width
                                visible: entry.isArmed
                                         || menu.outcomeVerb === entry.modelData.verb
                                text: {
                                    if (entry.isArmed)
                                        return entry.modelData.warning;
                                    if (menu.outcomeState === "pending")
                                        return qsTr("preguntando…");
                                    if (menu.outcomeState === "failed")
                                        return qsTr("el gestor de sesión rechazó la solicitud");
                                    return qsTr("el gestor de sesión lo aceptó");
                                }
                                color: menu.outcomeState === "failed"
                                       && menu.outcomeVerb === entry.modelData.verb
                                       ? backdropInk.danger : backdropInk.muted
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
}
