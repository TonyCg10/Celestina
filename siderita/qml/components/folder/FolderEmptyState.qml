import QtQuick
import org.celestina.siderita 1.0

Column {
    id: root

    required property var controller
    spacing: CelestinaTheme.spaceSm
    visible: !controller.loading
             && controller.errorText.length === 0
             && controller.entryNames.length === 0
             && !controller.searchRunning

    readonly property bool searchEmpty: controller.searchActive
                                                || controller.query.length > 0

    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        text: root.searchEmpty ? "Sin coincidencias" : "Carpeta vacía"
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontHeaderCollapsed
        font.weight: CelestinaTheme.weightMedium
    }

    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        text: root.searchEmpty
              ? "Prueba con otra búsqueda."
              : "No hay elementos que mostrar."
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
    }
}
