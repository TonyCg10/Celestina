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
    readonly property int selectedIndex: {
        var _ = activeController ? activeController.entryNames.length : 0
        return activeController && selectionCount === 1
               && activeController.selectedToken.length > 0
               ? activeController.indexForToken(activeController.selectedToken) : -1
    }

    readonly property string heading: selectionCount > 1 ? "SELECCIÓN"
                                          : selectedIndex >= 0 ? "ELEMENTO" : "CARPETA"
    readonly property string primaryText: selectionCount > 1
            ? selectionCount + " seleccionados"
            : selectedIndex >= 0 ? activeController.entryNames[selectedIndex]
            : entryCount + (entryCount === 1 ? " elemento" : " elementos")
    readonly property string secondaryText: selectionCount > 1 ? ""
            : selectedIndex >= 0 ? activeController.entryDetail(selectedIndex)
            : (activeController && activeController.folderSize.length > 0
               ? "Total " + activeController.folderSize : "")

    role: CelestinaSurface.Panel

    Column {
        x: 18
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width - 34
        spacing: 4

        Text {
            text: root.heading
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary
                                       * root.hostWindow.sidebarTextScale)
            font.letterSpacing: 1.4
            font.weight: CelestinaTheme.weightDemiBold
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

        Text {
            width: parent.width
            visible: root.secondaryText.length > 0
            text: root.secondaryText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary
                                       * root.hostWindow.sidebarTextScale)
            elide: Text.ElideRight
        }
    }
}
