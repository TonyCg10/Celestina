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
    // The level as the provider last published it — the truth, and never what
    // this row asked for. What the row *shows* between an ask and its answer
    // is `shownLevel`, and that distinction is the whole of the pacing below.
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

    // What this row asked for, and how fast it is allowed to ask again.
    //
    // These are two separate things, and treating them as one is what made a
    // dragged slider jump backwards. A level provider answers at its own pace
    // — `wpctl` costs a process per move, a monitor over DDC takes about a
    // second — and it also publishes readings nobody asked for: a poll, or the
    // read-back of a request from before this one. Those arrive *while* a drag
    // is still moving, and they describe where the device was, not where the
    // person is putting it.
    //
    // So the thumb follows what was asked for, and only a reading that
    // actually answers the ask — or the settle below, when nothing does — puts
    // it back under the provider's control. Meanwhile any reading at all is
    // enough to release the pacing: it means the round trip completed, so the
    // next position can go now instead of waiting out a timer.
    property int asked: -1
    // A request is travelling. One at a time, so a drag paces itself to what
    // the device can really do rather than queueing every position it crossed.
    property bool waiting: false
    // The newest position the drag reached while that request was travelling.
    property int queued: -1

    readonly property int shownLevel: root.asked >= 0 ? root.asked : root.level

    function ask(target) {
        const bounded = Math.max(0, Math.min(100, Math.round(target)));
        if (!root.enabled || !root.known || bounded === root.shownLevel)
            return;

        // The thumb goes where the person put it, whether or not the request
        // can leave yet.
        root.asked = bounded;
        settle.restart();
        if (root.waiting) {
            root.queued = bounded;
            return;
        }
        root.waiting = true;
        root.moved(bounded);
    }

    // The provider published something. That completes the round trip, which
    // is all the pacing needs; whether it answers what was asked is a
    // different question, and only an exact answer returns the thumb.
    function readingArrived() {
        root.waiting = false;
        if (root.queued >= 0) {
            const next = root.queued;
            root.queued = -1;
            root.waiting = true;
            root.moved(next);
            return;
        }
        if (root.asked >= 0 && root.level === root.asked)
            root.settled();
    }

    // The provider's reading is the truth again.
    function settled() {
        root.asked = -1;
        root.queued = -1;
        settle.stop();
    }

    // A notch lands on a round number rather than on `shown + step`: from 22,
    // five percent up is 25 and five percent down is 20. The same rule the
    // session's own step verbs apply, because it is the same gesture — what
    // the wheel asks for is a level, not an offset.
    function nudge(direction) {
        const size = Math.max(1, root.step);
        const from = root.shownLevel;
        root.ask(direction > 0
                 ? (Math.floor(from / size) + 1) * size
                 : Math.floor((from - 1) / size) * size);
    }

    onLevelChanged: root.readingArrived()

    // What a device that never confirms the exact ask leaves behind: a ceiling
    // `wpctl` applied, a monitor that refused the value, a request that was
    // simply lost. After this long with nothing asked and nothing answered,
    // whatever the provider says is what is true.
    Timer {
        id: settle

        interval: 1500
        onTriggered: root.settled()
    }

    implicitHeight: root.settledHeight
    Accessible.role: Accessible.Slider
    Accessible.name: root.known
                     ? qsTr("%1: %2 %").arg(root.label).arg(root.shownLevel)
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
                text: root.known ? qsTr("%1 %").arg(root.shownLevel) : root.unknownText
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
                value: root.known ? root.shownLevel : 0
                onMoved: (target) => root.ask(target)
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
