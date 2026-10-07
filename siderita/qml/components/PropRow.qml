import QtQuick
import org.celestina.siderita 1.0

// ─── PropRow ──────────────────────────────────────────────────────────────────
// One label/value row of the properties dialog: the label in grey on the
// left, the value on the right, and under the value whatever the host puts in
// `detail` (a volume's bar). A row without a value takes no room.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: propRow
    property string label: ""
    property string value: ""
    // Optional child placed under the value, the width of the value column.
    default property alias detail: detailSlot.data
    readonly property int labelWidth: 120

    visible: value.length > 0
    implicitHeight: visible
                    ? Math.max(CelestinaTheme.rowHeight,
                               propValue.implicitHeight + detailSlot.implicitHeight
                               + CelestinaTheme.spaceMd * 2)
                    : 0
    height: implicitHeight

    Text {
        id: propLabel
        x: CelestinaTheme.spaceMd
        y: CelestinaTheme.spaceMd
        width: propRow.labelWidth
        text: propRow.label
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
    }
    Text {
        id: propValue
        anchors.left: propLabel.right
        anchors.right: parent.right
        anchors.rightMargin: CelestinaTheme.spaceMd
        y: CelestinaTheme.spaceMd
        text: propRow.value
        textFormat: Text.PlainText
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        font.features: CelestinaTheme.fontFeaturesTabular
        wrapMode: Text.WrapAnywhere
    }
    Item {
        id: detailSlot
        anchors.left: propValue.left
        anchors.right: propValue.right
        anchors.top: propValue.bottom
        anchors.topMargin: detailSlot.implicitHeight > 0 ? CelestinaTheme.spaceSm : 0
        implicitHeight: childrenRect.height
        height: implicitHeight
    }
}
