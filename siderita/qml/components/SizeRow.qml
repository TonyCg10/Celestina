import QtQuick
import org.celestina.siderita 1.0

// ─── SizeRow ──────────────────────────────────────────────────────────────────
// One row of the sizes menu: label, slider and the value as a percentage.
// Each icon/text pair gets its own because enlarging text and enlarging icons
// are two different wishes, and a single slider forces a choice.
//
// The slider is the system's `CelestinaSlider`, not a restyled Qt Controls
// one: it brings the focus ring, the keyboard steps and the accessible role
// the copy lacked, and it is controlled — it shows `value` and reports where a
// person asked to go, which the host stores and binds back here.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: sizeRow
    property string label: ""
    property real value: 1
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

    CelestinaSlider {
        id: sizeSlider
        objectName: "sizeSlider"
        anchors.left: sizeRowLabel.right
        anchors.right: sizeRowValue.left
        anchors.rightMargin: 10
        anchors.verticalCenter: parent.verticalCenter
        // The track is 4 px and the handle 15; the target is the row's 30.
        height: CelestinaTheme.controlHeightXs
        value: sizeRow.value
        from: sizeRow.minValue
        to: sizeRow.maxValue
        step: 0.1

        Accessible.name: sizeRow.label

        onMoved: function(target) { sizeRow.moved(sizeRow.snap(target)) }
    }

    // Tenths, as the steps the menu has always offered: a drag reports every
    // pixel, and a scale of 1.0371 is not one a person chose. A bound is kept
    // as it is, and the rounded value is held inside the bounds: rounding a
    // 1.25 maximum gave 1.3, and a 0.75 minimum could never be reached.
    function snap(target) {
        if (target <= sizeRow.minValue)
            return sizeRow.minValue
        if (target >= sizeRow.maxValue)
            return sizeRow.maxValue
        const tenths = Math.round(target * 10) / 10
        return Math.max(sizeRow.minValue, Math.min(sizeRow.maxValue, tenths))
    }

    Text {
        id: sizeRowValue
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: 38
        horizontalAlignment: Text.AlignRight
        text: Math.round(sizeRow.value * 100) + "%"
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        // Tabular figures so the percentage does not shift width as it counts.
        font.features: CelestinaTheme.fontFeaturesTabular
    }
}
