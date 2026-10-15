// A section's name, and — when there is anything to choose between — the way
// into its device list, as one compact chevron rather than a wide button.
//
// Icon-first, the author's stated hierarchy (2026-08-12): a secondary action
// is an icon where the action applies, never a text button spanning the card.
// One device is the answer, not a question, so the chevron only exists when
// the list holds a real choice.
//
// The mark the author asked to be legible: the row in use carries a filled
// selected surface and a check, and a row under the pointer carries its own
// hover surface. Both are drawn by this file rather than left to a ghost
// button's faint tint, which on the dark glass was almost invisible.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

Item {
    id: root

    required property BackdropInk ink
    required property string label
    required property var devices
    required property bool expanded

    signal toggled()
    signal chosen(int id)

    readonly property bool offersChoice: root.devices.length > 1
    // Stated, for the same model arithmetic every card height uses.
    readonly property int labelBandHeight: CelestinaTheme.controlHeightXs
    readonly property int deviceBandHeight: CelestinaTheme.controlHeightXs
                                            + CelestinaTheme.spaceXs
    readonly property int settledHeight: root.labelBandHeight
                                         + (root.expanded && root.offersChoice
                                            ? root.devices.length
                                              * root.deviceBandHeight
                                            : 0)

    implicitHeight: root.settledHeight

    Column {
        id: column

        anchors.left: parent.left
        anchors.right: parent.right
        spacing: 0

        Item {
            width: parent.width
            height: root.labelBandHeight

            Text {
                anchors.left: parent.left
                anchors.right: chevron.visible ? chevron.left : parent.right
                anchors.rightMargin: chevron.visible ? CelestinaTheme.spaceSm : 0
                anchors.verticalCenter: parent.verticalCenter
                text: root.label
                textFormat: Text.PlainText
                color: root.ink.muted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
            }

            BackdropIconButton {
                id: chevron

                objectName: "celestina-device-picker-toggle"
                anchors.right: parent.right
                anchors.verticalCenter: parent.verticalCenter
                visible: root.offersChoice
                width: CelestinaTheme.controlHeightXs
                height: width
                ink: root.ink
                iconName: "chevron-down"
                // The glyph turns rather than swapping: there is no upward
                // chevron in the catalogue, and the turn says the same thing.
                // The circle behind it is rotation-invariant, so the whole
                // button may carry it.
                rotation: root.expanded ? 180 : 0
                helpText: root.expanded ? qsTr("Ocultar los dispositivos")
                                        : qsTr("Elegir el dispositivo")
                Accessible.name: helpText
                onClicked: root.toggled()
            }
        }

        Repeater {
            model: root.expanded && root.offersChoice ? root.devices : []

            delegate: Item {
                id: entry

                required property var modelData

                readonly property bool inUse: entry.modelData.default === true

                width: column.width
                height: root.deviceBandHeight

                Accessible.role: Accessible.RadioButton
                Accessible.name: entry.modelData.name
                Accessible.checked: entry.inUse
                Accessible.onPressAction: {
                    if (!entry.inUse)
                        root.chosen(entry.modelData.id);
                }

                // The state, said as a surface. Selected outranks hover so the
                // device in use never stops looking chosen while the pointer
                // crosses it.
                Rectangle {
                    anchors.fill: parent
                    anchors.bottomMargin: CelestinaTheme.spaceXs
                    radius: CelestinaTheme.radiusSm
                    color: entry.inUse ? root.ink.selectedFill
                                       : hover.hovered ? root.ink.hoverFill
                                                       : CelestinaTheme.clear
                    border.width: entry.inUse ? CelestinaTheme.borderHairline : 0
                    border.color: entry.inUse ? root.ink.divider
                                              : CelestinaTheme.clear
                }

                HoverHandler {
                    id: hover

                    cursorShape: entry.inUse ? Qt.ArrowCursor
                                             : Qt.PointingHandCursor
                }

                TapHandler {
                    enabled: !entry.inUse
                    onTapped: root.chosen(entry.modelData.id)
                }

                Text {
                    anchors.left: parent.left
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.right: mark.left
                    anchors.rightMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: -CelestinaTheme.spaceXs / 2
                    text: entry.modelData.name
                    textFormat: Text.PlainText
                    color: entry.inUse ? root.ink.primary : root.ink.muted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    elide: Text.ElideRight
                }

                CelestinaIcon {
                    id: mark

                    anchors.right: parent.right
                    anchors.rightMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: parent.verticalCenter
                    anchors.verticalCenterOffset: -CelestinaTheme.spaceXs / 2
                    width: CelestinaTheme.iconSm
                    height: width
                    visible: entry.inUse
                    name: "check"
                    fallbackName: "check"
                    tintOverride: root.ink.primary
                    Accessible.ignored: true
                }
            }
        }
    }
}
