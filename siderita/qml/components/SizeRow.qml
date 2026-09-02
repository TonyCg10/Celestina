import QtQuick
import QtQuick.Controls
import org.celestina.siderita 1.0

// ─── SizeRow ──────────────────────────────────────────────────────────────────
// Una fila del menú de tamaños: etiqueta, deslizador y el valor en porcentaje.
// Cada pareja icono/texto tiene la suya porque agrandar el texto y agrandar los
// iconos son dos deseos distintos, y un solo deslizador obliga a elegir.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: sizeRow
    property string label: ""
    property alias value: sizeSlider.value
    // Scale is literal: 1.0 = 100%. Icon rows provide tighter bounds; text
    // keeps the wider accessibility range.
    property real minValue: 0.2
    property real maxValue: 2.0
    signal moved(real v)

    implicitWidth: 252
    implicitHeight: 30

    Text {
        id: sizeRowLabel
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: 94
        text: sizeRow.label
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
        elide: Text.ElideRight
    }

    Slider {
        id: sizeSlider
        anchors.left: sizeRowLabel.right
        anchors.right: sizeRowValue.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        // The track is 4 px and the handle 15; the target is the row's 30.
        height: CelestinaTheme.controlHeightXs
        from: sizeRow.minValue
        to: sizeRow.maxValue
        stepSize: 0.1
        onMoved: sizeRow.moved(value)

        background: Rectangle {
            x: sizeSlider.leftPadding
            y: sizeSlider.topPadding + sizeSlider.availableHeight / 2 - height / 2
            width: sizeSlider.availableWidth
            height: CelestinaTheme.compLinearTrackHeight
            radius: height / 2
            color: CelestinaTheme.controlFill

            Rectangle {
                width: sizeSlider.visualPosition * parent.width
                height: parent.height
                radius: height / 2
                color: CelestinaTheme.accent
            }
        }

        handle: Rectangle {
            x: sizeSlider.leftPadding
               + sizeSlider.visualPosition * (sizeSlider.availableWidth - width)
            y: sizeSlider.topPadding + sizeSlider.availableHeight / 2 - height / 2
            width: CelestinaTheme.compSliderHandleSize
            height: CelestinaTheme.compSliderHandleSize
            radius: height / 2
            color: sizeSlider.pressed ? CelestinaTheme.accent : CelestinaTheme.text
            border.width: CelestinaTheme.borderHairline
            border.color: CelestinaTheme.dividerStrong
        }
    }

    Text {
        id: sizeRowValue
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: 38
        horizontalAlignment: Text.AlignRight
        text: Math.round(sizeSlider.value * 100) + "%"
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        // Tabular figures so the percentage does not shift width as it counts.
        font.features: CelestinaTheme.fontFeaturesTabular
    }
}
