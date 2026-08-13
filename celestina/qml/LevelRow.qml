// One thing with a level: a name, a slider, and optionally a button beside it.
//
// Shared by the audio and brightness cards because they are the same control
// with different words. Both are continuous quantities, so both are sliders
// rather than menu rows: a level is moved, not chosen.
//
// The wheel steps by whole units of `step` anywhere on the row, matching the
// panel control the row's own menu was opened from — one vocabulary for the
// same quantity, whether the pointer is on the bar or in the card.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    required property BackdropInk ink
    required property string label
    // The level as the provider last published it. The slider is never moved
    // by this file's own click: `moved` asks, and the next reading answers.
    required property int level
    property string iconName: ""
    property string secondaryText: ""
    // A row whose subject has no readable level says so instead of drawing a
    // slider parked at zero, which reads as "off" and is a different claim.
    property bool known: true
    property string unknownText: qsTr("sin respuesta")
    property int step: 5
    // The optional button at the trailing edge — muting, for the audio rows.
    property string actionIcon: ""
    property string actionHelpText: ""
    property bool actionSelected: false

    signal moved(int level)
    signal actionTriggered()

    // Every internal band has a stated height, so a card can add rows up from
    // its model instead of measuring its own laid-out content — which is the
    // binding loop that left cards parked over the bar.
    // Tall enough for the row title at its own line height, not for the icon
    // alone: stated at `iconSm` the band was shorter than the text it holds,
    // so every card summed heights that were smaller than what it drew and the
    // last row fell off the bottom.
    readonly property int nameBandHeight: CelestinaTheme.controlHeightXs
    readonly property int secondaryBandHeight: 16
    readonly property int settledHeight: root.nameBandHeight
                                         + CelestinaTheme.spaceXs
                                         + CelestinaTheme.controlHeightXs
                                         + (root.secondaryText.length > 0
                                            ? CelestinaTheme.spaceXs
                                              + root.secondaryBandHeight
                                            : 0)

    function nudge(direction) {
        if (!root.enabled || !root.known)
            return;
        const target = Math.max(0, Math.min(100, root.level + direction * root.step));
        if (target !== root.level)
            root.moved(target);
    }

    implicitHeight: root.settledHeight
    Accessible.role: Accessible.Slider
    Accessible.name: root.known
                     ? qsTr("%1: %2 %").arg(root.label).arg(root.level)
                     : qsTr("%1: %2").arg(root.label).arg(root.unknownText)
    Accessible.onIncreaseAction: root.nudge(1)
    Accessible.onDecreaseAction: root.nudge(-1)

    WheelHandler {
        // A notch is 120 eighths of a degree; a touchpad's finer scroll
        // accumulates rather than being dropped.
        property real steps: 0

        enabled: root.enabled && root.known
        acceptedDevices: PointerDevice.Mouse | PointerDevice.TouchPad
        onWheel: (event) => {
            steps += event.angleDelta.y / 120;
            while (steps >= 1) {
                steps -= 1;
                root.nudge(1);
            }
            while (steps <= -1) {
                steps += 1;
                root.nudge(-1);
            }
        }
    }

    Column {
        id: column

        anchors.left: parent.left
        anchors.right: parent.right
        spacing: CelestinaTheme.spaceXs

        Item {
            width: parent.width
            height: root.nameBandHeight

            CelestinaIcon {
                id: leadingIcon

                anchors.left: parent.left
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconSm
                height: width
                visible: root.iconName.length > 0
                name: root.iconName
                fallbackName: root.iconName
                tintOverride: root.enabled ? root.ink.primary : root.ink.muted
                Accessible.ignored: true
            }

            Text {
                id: nameText

                anchors.left: leadingIcon.visible ? leadingIcon.right : parent.left
                anchors.leftMargin: leadingIcon.visible ? CelestinaTheme.spaceSm : 0
                anchors.right: valueText.left
                anchors.rightMargin: CelestinaTheme.spaceSm
                anchors.verticalCenter: parent.verticalCenter
                text: root.label
                textFormat: Text.PlainText
                color: root.enabled ? root.ink.primary : root.ink.muted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                elide: Text.ElideRight
            }

            Text {
                id: valueText

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: root.known ? qsTr("%1 %").arg(root.level) : root.unknownText
                textFormat: Text.PlainText
                color: root.known ? root.ink.primary : root.ink.faint
                font.family: CelestinaTheme.sansFamily
                font.features: CelestinaTheme.fontFeaturesTabular
                font.pixelSize: CelestinaTheme.fontMini
                font.weight: CelestinaTheme.weightDemiBold
            }
        }

        Item {
            width: parent.width
            height: CelestinaTheme.controlHeightXs

            CelestinaSlider {
                id: slider

                anchors.left: parent.left
                anchors.right: action.visible ? action.left : parent.right
                anchors.rightMargin: action.visible ? CelestinaTheme.spaceSm : 0
                anchors.verticalCenter: parent.verticalCenter
                enabled: root.enabled && root.known
                from: 0
                to: 100
                step: root.step
                value: root.known ? root.level : 0
                onMoved: (target) => root.moved(Math.round(target))
            }

            BackdropIconButton {
                id: action

                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                visible: root.actionIcon.length > 0
                width: CelestinaTheme.controlHeightXs
                height: width
                ink: root.ink
                iconName: root.actionIcon
                helpText: root.actionHelpText
                enabled: root.enabled
                role: root.actionSelected ? CelestinaButton.Selected
                                          : CelestinaButton.Ghost
                onClicked: root.actionTriggered()
            }
        }

        Text {
            width: parent.width
            height: root.secondaryBandHeight
            visible: root.secondaryText.length > 0
            text: root.secondaryText
            textFormat: Text.PlainText
            color: root.ink.muted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontMini
            elide: Text.ElideRight
        }
    }
}
