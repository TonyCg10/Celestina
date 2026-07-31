import QtQuick
import org.celestina.siderita 1.0

// ─── RecentHeader ───────────────────────────────────────────────────────────
// Cabecera de la ubicación "Recientes": una pastilla que dice dónde estás y
// cuánto hay, y el camino de vuelta. Nada más — esa lista es del escritorio y
// Siderita sólo la lee. Controlador, cristal de fondo y escala de texto por
// propiedad; la vista de carpeta la posiciona y la muestra.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property Item backdrop     // topBar.activeView: cristal de la pastilla
    property real textScale: 1.0

    InfoPill {
        textScale: root.textScale
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        backdrop: root.backdrop
        iconName: "document-open-recent"
        iconFallback: "file"
        maxWidth: root.width - recentHeaderControls.width - 10
        text: "Recientes" + (root.controller.recentCount > 0
                             ? "  ·  " + root.controller.recentCount
                             : "  ·  sin elementos")
    }

    // Los botones flotan directamente sobre la lista y no viven dentro de
    // ninguna caja: sin este envoltorio, pulsar uno y arrastrar hacia el
    // contenido arrancaba el arrastre del archivo que tapan.
    Item {
        id: recentHeaderControls
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        width: recentHeaderControlsRow.width
        height: recentHeaderControlsRow.height

        CelestinaInputShield { }

        Row {
            id: recentHeaderControlsRow
            spacing: 8

            CelestinaButton {
                text: "Volver"
                role: CelestinaButton.Primary
                onClicked: root.controller.closeRecent()
            }
        }
    }
}
