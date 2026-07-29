import QtQuick
import org.celestina.siderita 1.0

// Selection/folder summary shown as a separate panel below the main sidebar.
// It reads the active controller but owns no navigation or domain state.
CelestinaSurface {
    id: root

    required property var hostWindow

    readonly property var activeController: hostWindow.activeController
    readonly property int selectionCount: activeController
                                                  ? activeController.selectionCount : 0
    readonly property int entryCount: activeController
                                              ? activeController.entryNames.length : 0
    readonly property bool virtualLocation: activeController
            && (activeController.searchActive || activeController.searchRunning
                || activeController.trashActive || activeController.recentActive)
    readonly property int selectedIndex: {
        var _ = activeController ? activeController.entryNames.length : 0
        return activeController && selectionCount === 1
               && activeController.selectedToken.length > 0
               ? activeController.indexForToken(activeController.selectedToken) : -1
    }
    readonly property bool hasSelectedEntry:
            selectedIndex >= 0 && selectedIndex < entryCount

    readonly property string heading: selectionCount > 1 ? "SELECCIÓN"
            : hasSelectedEntry ? "ELEMENTO"
            : activeController && activeController.trashActive ? "PAPELERA"
            : activeController && activeController.recentActive ? "RECIENTES"
            : activeController
              && (activeController.searchActive || activeController.searchRunning)
                    ? "BÚSQUEDA" : "CARPETA"
    readonly property string primaryText: selectionCount > 1
            ? selectionCount + " seleccionados"
            : hasSelectedEntry ? activeController.entryNames[selectedIndex]
            : entryCount + (entryCount === 1 ? " elemento" : " elementos")
    readonly property var detailLines: {
        if (selectionCount > 1)
            return []
        if (hasSelectedEntry)
            return activeController.entryInfo(selectedIndex)
        if (!virtualLocation && activeController
            && activeController.folderSize.length > 0)
            return ["Directo " + activeController.folderSize]
        return []
    }

    implicitHeight: Math.round(contentColumn.implicitHeight
                               + 28 * hostWindow.sidebarTextScale)

    role: CelestinaSurface.Panel

    Column {
        id: contentColumn
        x: 18
        y: Math.round(14 * root.hostWindow.sidebarTextScale)
        width: parent.width - 34
        spacing: 4

        CelestinaSectionLabel {
            text: root.heading
            size: CelestinaSectionLabel.Regular
            textScale: root.hostWindow.sidebarTextScale
        }

        Text {
            width: parent.width
            text: root.primaryText
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontRowTitle
                                       * root.hostWindow.sidebarTextScale)
            font.weight: CelestinaTheme.weightMedium
            elide: Text.ElideMiddle
        }

        Repeater {
            model: root.detailLines

            Text {
                required property string modelData

                width: contentColumn.width
                text: modelData
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary
                                           * root.hostWindow.sidebarTextScale)
                elide: Text.ElideRight
            }
        }
    }
}
