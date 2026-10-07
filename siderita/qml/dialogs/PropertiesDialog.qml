import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Properties / Get-Info panel ──────────────────────────────────
CelestinaModalLayer {
    id: propertiesView
    property var controller
    property var owner
    property var backdrop   // mainPanel: el fondo que difumina el cristal
    property var panel      // mainPanel: apariencia semántica y por ruta
    // SideritaUsage: a folder's occupation, scanned while the dialog is open.
    property var usage
    readonly property string iconKind: propertiesView.controller.propIsDir
                                               ? "directory"
                                               : propertiesView.controller.propSymlink.length > 0
                                                 ? "symlink" : "file"
    anchors.fill: parent
    z: 68
    shown: propertiesView.controller.propertiesPending
    onDismissRequested: propertiesView.controller.closeProperties()

    // One scan per open folder, cancelled when the dialog closes, so no
    // thread keeps reading the disk behind a closed dialog.
    onShownChanged: propertiesView.syncUsage()
    Connections {
        target: propertiesView.controller
        function onPropKeyChanged() { propertiesView.syncUsage() }
        function onPropIsDirChanged() { propertiesView.syncUsage() }
    }

    // The hub is shared with the quick look: whoever opened it last owns
    // it, a close from the other is ignored, and a section only presents the
    // hub while its own modal owns it.
    readonly property string usageOwner: "properties"
    readonly property bool ownsUsage: !!propertiesView.usage
                                      && propertiesView.usage.owner === propertiesView.usageOwner
    function syncUsage() {
        if (!propertiesView.usage)
            return
        if (propertiesView.shown && propertiesView.controller.propIsDir)
            propertiesView.usage.open(propertiesView.controller.propKey, propertiesView.usageOwner)
        else
            propertiesView.usage.close(propertiesView.usageOwner)
    }

    readonly property string folderSizeText: !propertiesView.ownsUsage ? ""
        : propertiesView.usage.mode === "analysed" ? folderUsageSection.bytesText(propertiesView.usage.totalBytes)
        : propertiesView.usage.mode === "failed" ? qsTr("No legible")
        : qsTr("Calculando…")

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(propertiesView.controller.propIsDir ? 760 : 500, propertiesView.owner.width - 48)
        height: Math.min(propertiesColumn.implicitHeight + propHeading.height + propSubheading.height + 96
                         + (folderUsageSection.visible ? folderUsageSection.height + 12 : 0),
                         propertiesView.owner.height - 64)
        backdropSource: propertiesView.backdrop
        // (not transform-scaled — a scale transform desynced the glass backdrop)
        Accessible.role: Accessible.Dialog
        Accessible.name: "Propiedades"

        MouseArea { anchors.fill: parent }

        CelestinaIcon {
            id: propIcon
            x: 18
            y: 18
            width: CelestinaTheme.iconMd
            height: CelestinaTheme.iconMd
            name: propertiesView.panel.icons.mediaIconName(propertiesView.iconKind, "",
                                      propertiesView.controller.propPath)
            fallbackName: propertiesView.controller.propIsDir ? "folder" : "file"
            tone: propertiesView.panel.icons.entryIconTone(propertiesView.iconKind)
            tintOverride: propertiesView.panel.icons.iconTint(propertiesView.controller.propPath)
        }

        Text {
            id: propHeading
            anchors.left: propIcon.right
            anchors.leftMargin: 12
            anchors.right: parent.right
            anchors.rightMargin: 18
            y: 18
            text: propertiesView.controller.propName
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideMiddle
        }

        // What it is and where: the kind, then the path, in one quiet line.
        Text {
            id: propSubheading
            anchors.left: propHeading.left
            anchors.right: propHeading.right
            anchors.top: propHeading.bottom
            anchors.topMargin: CelestinaTheme.spaceXs
            text: propertiesView.controller.propKind + " · " + propertiesView.controller.propPath
            textFormat: Text.PlainText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            elide: Text.ElideMiddle
        }

        Flickable {
            id: propFlick
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            anchors.top: propSubheading.bottom
            anchors.topMargin: CelestinaTheme.spaceLg
            anchors.bottom: folderUsageSection.visible ? folderUsageSection.top : propButtons.top
            anchors.bottomMargin: 12
            clip: true
            contentHeight: propertiesColumn.implicitHeight
            boundsBehavior: Flickable.StopAtBounds

            CelestinaWheelScroll { view: propFlick }

            Column {
                id: propertiesColumn
                width: propFlick.width

                // The facts a person opens this dialog for, in one quiet box:
                // what it weighs, how much room is left, when it changed and
                // who may touch it. Kind, MIME and access time are not here:
                // the heading says what it is, and an access time changes the
                // moment the dialog reads it.
                CelestinaSurface {
                    width: parent.width
                    role: CelestinaSurface.Content
                    implicitHeight: propertyRows.implicitHeight

                    Column {
                        id: propertyRows
                        width: parent.width

                        PropRow {
                            width: parent.width
                            label: qsTr("Tamaño")
                            value: propertiesView.controller.propIsDir ? propertiesView.folderSizeText
                                                                      : propertiesView.controller.propSize
                        }
                        PropRow {
                            width: parent.width
                            label: qsTr("Espacio libre")
                            value: propertiesView.controller.propVolumeTotal > 0
                                   ? qsTr("%1 libres de %2")
                                         .arg(folderUsageSection.bytesText(propertiesView.controller.propVolumeFree))
                                         .arg(folderUsageSection.bytesText(propertiesView.controller.propVolumeTotal))
                                   : ""

                            // How full the volume is: a thin track, the used
                            // share in the accent.
                            Rectangle {
                                width: parent.width
                                height: CelestinaTheme.compLinearTrackHeight
                                radius: CelestinaTheme.radiusPill
                                color: CelestinaTheme.divider

                                Rectangle {
                                    width: propertiesView.controller.propVolumeTotal > 0
                                           ? Math.round(parent.width * Math.max(0, Math.min(1,
                                                 1 - propertiesView.controller.propVolumeFree / propertiesView.controller.propVolumeTotal)))
                                           : 0
                                    height: parent.height
                                    radius: CelestinaTheme.radiusPill
                                    color: CelestinaTheme.accent
                                }
                            }
                        }
                        PropRow {
                            width: parent.width
                            label: qsTr("Modificado")
                            value: propertiesView.controller.propModified
                        }
                        PropRow {
                            width: parent.width
                            label: qsTr("Permisos")
                            value: propertiesView.controller.propPermissions.length > 0
                                   ? propertiesView.controller.propPermissions + " · " + propertiesView.controller.propOwner
                                   : ""
                        }
                        PropRow {
                            width: parent.width
                            label: qsTr("Enlace a")
                            value: propertiesView.controller.propSymlink
                        }
                    }
                }
            }
        }

        // A folder's occupation: crumbs, totals, list and treemap.
        FolderUsage {
            id: folderUsageSection
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            anchors.bottom: propButtons.top
            anchors.bottomMargin: 12
            height: 280
            visible: propertiesView.controller.propIsDir && propertiesView.ownsUsage
            usage: propertiesView.usage
            controller: propertiesView.controller
            onNavigated: propertiesView.controller.closeProperties()
        }

        Row {
            id: propButtons
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16

            CelestinaButton {
                text: "Cerrar"
                role: CelestinaButton.Primary
                onClicked: propertiesView.controller.closeProperties()
            }
        }
    }
}
