import QtQuick
import org.celestina.siderita 1.0

// ─── SearchBar ──────────────────────────────────────────────────────────────
// Cabecera de la ubicación "búsqueda": una pastilla con la consulta y el
// resumen, y los botones Detener / Cerrar. Los resultados son la vista de
// contenido. El controlador, el cristal de fondo (la vista activa) y la escala
// de texto llegan por propiedad; la vista de carpeta la posiciona y la muestra
// según el estado de búsqueda.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    property Item backdrop     // topBar.activeView: cristal de la pastilla
    property real textScale: 1.0

    InfoPill {
        id: searchBarLabel
        textScale: root.textScale
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        backdrop: root.backdrop
        iconName: "edit-find"
        iconFallback: "file"
        maxWidth: root.width - searchBarControls.width - 10
        text: root.controller.searchRunning
              ? "Buscando «" + root.controller.searchQuery + "»…"
              : "«" + root.controller.searchQuery + "» · " + root.controller.searchSummary
    }

    Row {
        id: searchBarControls
        anchors.right: parent.right
        anchors.rightMargin: 8
        anchors.verticalCenter: parent.verticalCenter
        spacing: 8

        CelestinaButton {
            text: "Detener"
            visible: root.controller.searchRunning
            onClicked: root.controller.cancelSearch()
        }
        CelestinaButton {
            text: "Cerrar"
            onClicked: root.controller.closeSearch()
        }
    }
}
