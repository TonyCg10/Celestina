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
    readonly property string iconKind: controller.propIsDir
                                               ? "directory"
                                               : controller.propSymlink.length > 0
                                                 ? "symlink" : "file"
    anchors.fill: parent
    z: 68
    shown: controller.propertiesPending
    onDismissRequested: controller.closeProperties()

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
        width: Math.min(propertiesView.controller.propIsDir ? 760 : 500, owner.width - 48)
        height: Math.min(propertiesColumn.implicitHeight + propHeading.height + 90
                         + (folderUsageSection.visible ? folderUsageSection.height + 12 : 0),
                         owner.height - 64)
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
            name: panel.icons.mediaIconName(propertiesView.iconKind, "",
                                      controller.propPath)
            fallbackName: controller.propIsDir ? "folder" : "file"
            tone: panel.icons.entryIconTone(propertiesView.iconKind)
            tintOverride: panel.icons.iconTint(controller.propPath)
        }

        Text {
            id: propHeading
            anchors.left: propIcon.right
            anchors.leftMargin: 12
            anchors.right: parent.right
            anchors.rightMargin: 18
            y: 20
            text: controller.propName
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideMiddle
        }

        Flickable {
            id: propFlick
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            anchors.top: propIcon.bottom
            anchors.topMargin: 14
            anchors.bottom: folderUsageSection.visible ? folderUsageSection.top : propButtons.top
            anchors.bottomMargin: 12
            clip: true
            contentHeight: propertiesColumn.implicitHeight
            boundsBehavior: Flickable.StopAtBounds

            Column {
                id: propertiesColumn
                width: propFlick.width

                PropRow { width: parent.width; label: "Ruta"; value: controller.propPath }
                PropRow { width: parent.width; label: "Tipo"; value: controller.propKind }
                PropRow {
                    width: parent.width
                    label: "Enlace a"
                    value: controller.propSymlink
                }
                PropRow { width: parent.width; label: "MIME"; value: controller.propMime }
                PropRow {
                    width: parent.width
                    label: "Tamaño"
                    value: propertiesView.controller.propIsDir ? propertiesView.folderSizeText
                                                              : propertiesView.controller.propSize
                }
                PropRow {
                    width: parent.width
                    label: "Permisos"
                    value: controller.propPermissions
                }
                PropRow {
                    width: parent.width
                    label: "Propietario"
                    value: controller.propOwner
                }
                PropRow {
                    width: parent.width
                    label: "Modificado"
                    value: controller.propModified
                }
                PropRow {
                    width: parent.width
                    label: "Accedido"
                    value: controller.propAccessed
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
                onClicked: controller.closeProperties()
            }
        }
    }
}
