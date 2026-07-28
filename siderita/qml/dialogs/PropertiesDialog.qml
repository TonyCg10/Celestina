import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Properties / Get-Info panel ──────────────────────────────────
Rectangle {
    id: propertiesView
    property var controller
    property var owner
    property var backdrop   // mainPanel: el fondo que difumina el cristal
    anchors.fill: parent
    z: 68
    readonly property bool shown: controller.propertiesPending
    // Fades rather than pops. Opacity only: a scale transform on a
    // glass surface desyncs its backdrop sampling (see a995619), so the
    // motion here never touches geometry.
    visible: opacity > 0.01
    opacity: shown ? 1 : 0
    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }
    color: CelestinaTheme.scrim

    MouseArea {
        anchors.fill: parent
        onClicked: controller.closeProperties()
    }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            controller.closeProperties()
            event.accepted = true
        }
    }
    focus: propertiesView.shown

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

        IconImage {
            id: propIcon
            x: 18
            y: 18
            width: CelestinaTheme.iconMd
            height: CelestinaTheme.iconMd
            name: controller.propIsDir ? "folder" : "text-x-generic"
            source: CelestinaTheme.fallbackIcon(
                        controller.propIsDir ? "folder" : "file")
            color: controller.propIsDir ? CelestinaTheme.accent
                                        : CelestinaTheme.textMuted
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
                primary: true
                onClicked: controller.closeProperties()
            }
        }
    }
}
