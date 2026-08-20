import QtQuick
import org.celestina.siderita 1.0

// Centered One UI heading. FolderView owns the reveal gesture; this component
// only animates between the compact title and a complete metadata summary.
Item {
    id: root

    required property var controller
    required property var hostWindow
    required property bool shortcutActive
    property bool compact: false
    // Retired is a third state, past compact: the heading is not there at all
    // and the listing takes its band. Compact remains what a folder shows by
    // default — the mistake this replaces made "compact" mean "gone", which
    // left the window with no title at all and the rows sliding under the bars.
    property bool retired: false
    property real compactProgress: compact ? 1 : 0
    property real retiredProgress: retired ? 1 : 0
    signal phoneMediaRequested(int index)

    readonly property int phoneIndex: {
        if (controller.trashActive || controller.recentActive
                || controller.searchActive || controller.searchRunning)
            return -1
        controller.phoneRevision
        const current = controller.currentPathKey.replace(/\/+$/, "")
        for (let index = 0; index < controller.phoneMounts.length; ++index) {
            const mount = controller.phoneMounts[index].replace(/\/+$/, "")
            if (mount.length > 0 && mount === current)
                return index
        }
        return -1
    }
    readonly property bool phoneLocation: phoneIndex >= 0
    readonly property var phoneInfo: {
        controller.phoneRevision
        return phoneLocation ? controller.phoneInfo(phoneIndex) : []
    }
    readonly property bool phoneConnected:
            phoneInfo.length > 3 && phoneInfo[3] === "1"

    Behavior on compactProgress {
        NumberAnimation {
            duration: CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }
    }

    Behavior on retiredProgress {
        NumberAnimation {
            duration: CelestinaTheme.motionNormal
            easing.type: CelestinaTheme.easeStandard
        }
    }

    readonly property string locationName: {
        if (controller.trashActive)
            return "Papelera"
        if (controller.recentActive)
            return "Recientes"
        if (controller.searchActive || controller.searchRunning)
            return "Resultados"
        // Re-evaluate when Magnetita republishes its parallel device lists.
        controller.phoneNames.length
        controller.phoneMounts.length
        // ADR 0008: the label comes from the adapter, which owns both the
        // decode and the phone-name substitution. No path arithmetic here.
        if (controller.currentPathKey.length === 0)
            return "Inicio"
        return controller.displayLocationName(controller.currentPathKey)
    }

    readonly property string contextLabel: controller.trashActive
                                                   ? "PAPELERA"
                                           : controller.recentActive
                                                   ? "ACTIVIDAD RECIENTE"
                                           : controller.searchActive
                                             || controller.searchRunning
                                                   ? "BÚSQUEDA"
                                           : root.phoneLocation ? "MÓVIL"
                                                   : "UBICACIÓN"

    readonly property bool virtualLocation:
            controller.trashActive || controller.recentActive
            || controller.searchActive || controller.searchRunning

    function countLabel(value, singular, plural) {
        return value + " " + (value === 1 ? singular : plural)
    }

    readonly property string primaryMetadata: {
        if (controller.searchActive || controller.searchRunning)
            return controller.searchSummary.length > 0
                   ? controller.searchSummary.toUpperCase()
                   : countLabel(controller.entryNames.length,
                                "RESULTADO", "RESULTADOS")
        if (controller.recentActive)
            return countLabel(controller.recentCount, "ELEMENTO", "ELEMENTOS")
        if (controller.trashActive)
            return countLabel(controller.entryNames.length,
                              "ELEMENTO", "ELEMENTOS")

        const visible = controller.folderVisibleCount
        const total = controller.folderTotalCount
        const parts = []
        if (visible === total) {
            parts.push(countLabel(total, "ELEMENTO", "ELEMENTOS"))
        } else {
            parts.push(visible + " VISIBLES DE "
                       + countLabel(total, "ELEMENTO", "ELEMENTOS"))
        }
        parts.push(countLabel(controller.folderDirectoryCount,
                              "CARPETA", "CARPETAS"))
        parts.push(countLabel(controller.folderFileCount,
                              "ARCHIVO", "ARCHIVOS"))
        if (controller.folderHiddenCount > 0)
            parts.push(countLabel(controller.folderHiddenCount,
                                  "OCULTO", "OCULTOS"))
        if (controller.folderSize.length > 0)
            parts.push("TAMAÑO DIRECTO " + controller.folderSize)
        return parts.join("  ·  ")
    }

    readonly property string secondaryMetadata: {
        if (virtualLocation)
            return ""
        const parts = []
        if (controller.folderModified.length > 0)
            parts.push("MODIFICADA " + controller.folderModified)
        if (controller.folderAccessed.length > 0)
            parts.push("ACCEDIDA " + controller.folderAccessed)
        if (controller.folderCreated.length > 0)
            parts.push("CREADA " + controller.folderCreated)
        return parts.join("  ·  ")
    }

    readonly property real expandedHeight:
            secondaryMetadata.length > 0 ? 116 : 98
    readonly property real compactHeight: 60
    height: Math.round((expandedHeight
                        + (compactHeight - expandedHeight) * compactProgress)
                       * (1 - retiredProgress))
    opacity: 1 - retiredProgress
    visible: opacity > 0.01
    // Nothing may spill out of a band that is closing.
    clip: true

    Column {
        x: root.phoneLocation ? 12 : 6
        y: (parent.height - implicitHeight) / 2
        width: root.phoneLocation
               ? Math.max(0, parent.width - mediaButton.width - 42)
               : Math.max(0, parent.width - 12)
        spacing: Math.round(CelestinaTheme.spaceXs
                            * (1 - root.compactProgress))

        CelestinaSectionLabel {
            width: parent.width
            height: implicitHeight * (1 - root.compactProgress)
            visible: opacity > 0.01
            opacity: 1 - root.compactProgress
            text: root.contextLabel
            textScale: root.hostWindow.interfaceTextScale
            horizontalAlignment: root.phoneLocation
                                 ? Text.AlignLeft : Text.AlignHCenter
        }

        Text {
            width: parent.width
            text: root.locationName.toUpperCase()
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round((CelestinaTheme.fontHeaderExpanded
                                        + (CelestinaTheme.fontHeaderCollapsed
                                           - CelestinaTheme.fontHeaderExpanded)
                                          * root.compactProgress)
                                       * root.hostWindow.interfaceTextScale)
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideMiddle
            horizontalAlignment: root.phoneLocation
                                 ? Text.AlignLeft : Text.AlignHCenter
        }

        Text {
            width: parent.width
            height: implicitHeight * (1 - root.compactProgress)
            visible: opacity > 0.01
            opacity: 1 - root.compactProgress
            text: root.primaryMetadata
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption
                                       * root.hostWindow.interfaceTextScale)
            elide: Text.ElideRight
            horizontalAlignment: root.phoneLocation
                                 ? Text.AlignLeft : Text.AlignHCenter
        }

        Text {
            width: parent.width
            height: implicitHeight * (1 - root.compactProgress)
            visible: opacity > 0.01 && text.length > 0
            opacity: 1 - root.compactProgress
            text: root.secondaryMetadata
            color: CelestinaTheme.textFaint
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontMini
                                       * root.hostWindow.interfaceTextScale)
            elide: Text.ElideRight
            horizontalAlignment: root.phoneLocation
                                 ? Text.AlignLeft : Text.AlignHCenter
        }
    }

    PhoneMediaButton {
        id: mediaButton

        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        // Present in both heading states — expanded and compact — and handed
        // over to the path bar only once the heading itself is gone.
        visible: root.phoneLocation && root.retiredProgress < 0.5
        connected: root.phoneConnected
        onClicked: root.phoneMediaRequested(root.phoneIndex)
    }

    Shortcut {
        sequence: "Alt+M"
        enabled: root.shortcutActive && root.phoneLocation && root.phoneConnected
        onActivated: root.phoneMediaRequested(root.phoneIndex)
    }
}
