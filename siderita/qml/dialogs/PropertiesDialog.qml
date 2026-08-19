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
    readonly property string iconKind: controller.propIsDir
                                               ? "directory"
                                               : controller.propSymlink.length > 0
                                                 ? "symlink" : "file"
    anchors.fill: parent
    z: 68
    shown: controller.propertiesPending
    onDismissRequested: controller.closeProperties()

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(500, owner.width - 48)
        height: Math.min(propertiesColumn.implicitHeight + propHeading.height + 90,
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
            anchors.bottom: propButtons.top
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
                PropRow { width: parent.width; label: "Tamaño"; value: controller.propSize }
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
