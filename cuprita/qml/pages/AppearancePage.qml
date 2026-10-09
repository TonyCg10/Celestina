pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.cuprita 1.0
import "../components"

// The Appearance section: the suite's shared appearance, edited here and
// nowhere else. Reduced motion and the text size; every open window of the
// suite, this one included, follows the file the controller writes.
Item {
    id: page

    // AppearanceController: the two values and their commands.
    required property var controller

    Accessible.name: qsTr("Apariencia")

    // The text sizes, in the file's spelling, smallest first.
    readonly property var scales: [
        { key: "compact", label: qsTr("Compacto") },
        { key: "normal", label: qsTr("Normal") },
        { key: "large", label: qsTr("Grande") },
        { key: "larger", label: qsTr("Muy grande") }
    ]

    function scaleIndex(key) {
        for (let i = 0; i < page.scales.length; ++i)
            if (page.scales[i].key === key)
                return i
        return 1
    }

    PageScroll {
        anchors.fill: parent

        SectionCard {
            id: card
            width: parent.width
            title: qsTr("Apariencia")

            LoadingLine {
                objectName: "appearanceLoading"
                visible: !page.controller.loaded
                text: qsTr("Leyendo la apariencia…")
            }

            SettingRow {
                objectName: "reducedMotionRow"
                visible: page.controller.loaded
                inset: card.rowInset
                width: parent.width
                label: qsTr("Reducir el movimiento")
                // The environment forces it on; the file cannot turn it off.
                checked: page.controller.forcedByEnvironment || page.controller.reducedMotion
                enabled: !page.controller.forcedByEnvironment
                hint: page.controller.forcedByEnvironment ? qsTr("Forzado por el entorno") : ""
                onToggled: function(on) { page.controller.setReducedMotion(on) }
            }

            Item {
                objectName: "textScaleRow"
                visible: page.controller.loaded
                width: parent.width
                // The label above, the four sizes below it: side by side
                // they would not fit the narrowest window.
                height: CelestinaTheme.rowHeight + choice.height + CelestinaTheme.spaceMd

                RowDivider { inset: card.rowInset }

                Text {
                    id: scaleLabel
                    anchors.left: parent.left
                    anchors.leftMargin: card.rowInset
                    anchors.right: parent.right
                    anchors.rightMargin: card.rowInset
                    height: CelestinaTheme.rowHeight
                    verticalAlignment: Text.AlignVCenter
                    text: qsTr("Tamaño del texto")
                    elide: Text.ElideRight
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                }

                // Left/Right walk the sizes; each step is saved at once and
                // the choice moves when the controller reads it back.
                ScaleChoice {
                    id: choice
                    objectName: "textScaleChoice"
                    anchors.left: parent.left
                    anchors.leftMargin: card.rowInset
                    anchors.top: scaleLabel.bottom
                    // Never past the card's edge, whatever the text size.
                    width: Math.min(implicitWidth, parent.width - card.rowInset * 2)
                    model: page.scales
                    helpText: qsTr("Tamaño del texto")
                    currentIndex: page.scaleIndex(page.controller.textScale)
                    onActivated: function(index) {
                        page.controller.setTextScale(page.scales[index].key)
                    }
                }
            }
        }
    }
}
