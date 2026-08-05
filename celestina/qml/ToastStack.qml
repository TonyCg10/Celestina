// The corner where this session's notifications appear.
//
// A toast is a glance, not a workspace: it shows who is talking, what they
// said and the buttons they offered, and it leaves when the server says it has.
// Nothing here decides how long that is — the shell's notification server owns
// every rule about expiry, replacement and caps, and this surface paints the
// list it publishes.
//
// It never takes the keyboard, so its buttons are reachable by pointer here and
// by keyboard in the notification centre. That is the deliberate split: a
// surface that grabbed focus every time an application spoke would interrupt
// typing, which is the one thing a notification must not do.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: stack

    // Each entry carries `id`, `app`, `summary`, `body`, `urgency`, `read` and
    // an `actions` list of `{key, label}`. `var` is necessary: QML has no typed
    // map-list.
    required property var toasts
    required property var providerSource
    required property bool reducedMotion

    readonly property int cardWidth: 380
    readonly property int cardSpacing: CelestinaTheme.spaceSm

    width: cardWidth
    height: column.implicitHeight
    color: CelestinaTheme.clear
    title: qsTr("Celestina notifications")

    Component.onCompleted: CelestinaTheme.reducedMotion = stack.reducedMotion

    function dismiss(id) {
        if (stack.providerSource)
            stack.providerSource.sendCommand("notifications", "dismiss", {"id": id});
    }

    function invoke(id, key) {
        if (stack.providerSource) {
            stack.providerSource.sendCommand("notifications", "invoke",
                                             {"id": id, "action": key});
        }
    }

    Column {
        id: column

        width: parent.width
        spacing: stack.cardSpacing

        Repeater {
            model: stack.toasts

            delegate: GlassCard {
                id: card

                required property var modelData

                readonly property bool critical: card.modelData.urgency === "critical"
                readonly property string spokenText: qsTr("%1: %2. %3")
                    .arg(card.modelData.app)
                    .arg(card.modelData.summary)
                    .arg(card.modelData.body)

                width: stack.cardWidth
                implicitHeight: body.implicitHeight + CelestinaTheme.spaceLg * 2
                backdropSource: null
                Accessible.role: Accessible.Notification
                Accessible.name: card.spokenText

                // A critical notification is the one case where the surface
                // says so on its own: the server will never time it out, so a
                // person needs to see that it is different.
                Rectangle {
                    anchors.left: parent.left
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.margins: CelestinaTheme.spaceXs
                    width: CelestinaTheme.spaceXs
                    radius: CelestinaTheme.radiusPill
                    visible: card.critical
                    color: CelestinaTheme.danger
                }

                Column {
                    id: body

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.spaceLg
                    spacing: CelestinaTheme.spaceXs

                    Row {
                        width: parent.width
                        spacing: CelestinaTheme.spaceSm

                        Text {
                            id: appLabel

                            width: parent.width - dismissButton.width - parent.spacing
                            text: card.modelData.app
                            color: CelestinaTheme.textMuted
                            elide: Text.ElideRight
                            font.family: CelestinaTheme.sansFamily
                            font.pixelSize: CelestinaTheme.fontCaption
                        }

                        CelestinaIconButton {
                            id: dismissButton

                            iconName: "x"
                            // Dismissing is this person having dealt with it,
                            // which is not what a timeout means.
                            helpText: qsTr("Dismiss this notification")
                            onClicked: stack.dismiss(card.modelData.id)
                        }
                    }

                    Text {
                        width: parent.width
                        text: card.modelData.summary
                        color: CelestinaTheme.text
                        elide: Text.ElideRight
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        font.weight: CelestinaTheme.weightDemiBold
                    }

                    Text {
                        width: parent.width
                        visible: card.modelData.body.length > 0
                        text: card.modelData.body
                        color: CelestinaTheme.textMuted
                        wrapMode: Text.WordWrap
                        maximumLineCount: 3
                        elide: Text.ElideRight
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }

                    Row {
                        width: parent.width
                        visible: card.modelData.actions.length > 0
                        spacing: CelestinaTheme.spaceSm

                        Repeater {
                            model: card.modelData.actions

                            delegate: CelestinaButton {
                                required property var modelData

                                text: modelData.label
                                onClicked: stack.invoke(card.modelData.id, modelData.key)
                            }
                        }
                    }
                }

                // Arriving is worth a movement; reduced motion keeps the toast
                // and drops the travel.
                opacity: 1
                Behavior on opacity {
                    enabled: !CelestinaTheme.reducedMotion
                    NumberAnimation {
                        duration: CelestinaTheme.motionFast
                        easing.type: CelestinaTheme.easeStandard
                    }
                }
            }
        }
    }
}
