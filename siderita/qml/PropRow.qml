import QtQuick
import org.celestina.siderita 1.0

// ─── PropRow ──────────────────────────────────────────────────────────────────
// Una fila etiqueta/valor del panel de propiedades. El valor se puede
// seleccionar con el ratón: una ruta que no se puede copiar es una ruta que hay
// que teclear.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: propRow
    property string label: ""
    property string value: ""
    visible: value.length > 0
    implicitHeight: visible ? Math.max(propValue.implicitHeight, 18) + 7 : 0
    height: implicitHeight

    Text {
        id: propLabel
        y: 3
        width: 104
        text: propRow.label
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
    }
    Text {
        id: propValue
        anchors.left: propLabel.right
        anchors.leftMargin: 8
        y: 3
        width: propRow.width - propLabel.width - 8
        text: propRow.value
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
        wrapMode: Text.WrapAnywhere
    }
}
