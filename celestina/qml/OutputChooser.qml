import QtQuick
import QtQuick.Window
import CelestinaStyle 1.0

// ─── OutputChooser ────────────────────────────────────────────────────────────
// "¿Qué pantalla quieres compartir?" — el diálogo que sale cuando una aplicación
// pide capturar la pantalla.
//
// No es un portal: `xdg-desktop-portal-wlr` no trae diálogo propio, sino que
// ejecuta un comando y se queda con el nombre de salida que ese comando
// imprima. Eso convierte al selector en una pieza reemplazable, y ésta es la
// nuestra — el primer trozo de la sesión que viste el lenguaje de Celestina sin
// que el shell tenga que servir el portal todavía.
//
// Contrato con el backend: escribir `CELESTINA-OUTPUT:<nombre>` y salir con 0.
// El envoltorio (`scripts/output-chooser.sh`) lo traduce a stdout, porque QML
// sólo puede escribir por el canal de diagnóstico.
// ──────────────────────────────────────────────────────────────────────────────
Window {
    id: chooser

    // La tarjeta se dimensiona sola: cabecera + una fila por salida + pie. La
    // *ventana* pide ese tamaño, pero un compositor con mosaico (niri) puede dar
    // otro, así que la tarjeta va centrada y a tamaño fijo dentro de ella — así
    // se ve igual la coloque donde la coloque.
    readonly property int rowHeight: 74
    readonly property int cardWidth: 560
    readonly property int cardHeight: 118 + Qt.application.screens.length * (rowHeight + 8) + 58
    width: cardWidth
    height: cardHeight
    visible: true
    color: "transparent"
    flags: Qt.Dialog | Qt.FramelessWindowHint
    title: "Compartir pantalla"

    property int selected: 0

    function choose(index) {
        const screens = Qt.application.screens
        if (index < 0 || index >= screens.length)
            return
        // El nombre del conector (DP-2, HDMI-A-1) es lo que espera el backend.
        console.log("CELESTINA-OUTPUT:" + screens[index].name)
        Qt.exit(0)
    }
    function cancel() {
        // Sin nombre y con código distinto de cero: el backend lo lee como
        // "el usuario no quiere compartir", que no es lo mismo que un fallo.
        Qt.exit(1)
    }

    // El panel: el mismo cristal, radio y borde encendido que el resto de la
    // suite. Sin GlassSurface porque aquí no hay nada detrás que desenfocar —
    // es una ventana suelta, no una superficie sobre contenido propio.
    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(chooser.cardWidth, chooser.width - 20)
        height: Math.min(chooser.cardHeight, chooser.height - 20)
        radius: CelestinaTheme.radiusLg
        color: CelestinaTheme.surfaceStrong
        border.width: 1
        border.color: CelestinaTheme.border

        Text {
            id: heading
            x: 22
            y: 18
            text: "Compartir pantalla"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCallout
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            id: subheading
            x: 22
            y: heading.y + heading.height + 2
            width: parent.width - 44
            text: Qt.application.screens.length > 1
                  ? "Elige qué salida verá la aplicación."
                  : "Se compartirá esta salida."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        ListView {
            id: list
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: subheading.bottom
            anchors.bottom: buttons.top
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            anchors.topMargin: 14
            anchors.bottomMargin: 12
            clip: true
            spacing: 8
            model: Qt.application.screens
            currentIndex: chooser.selected
            boundsBehavior: Flickable.StopAtBounds
            focus: true

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    chooser.cancel()
                } else if (event.key === Qt.Key_Down) {
                    chooser.selected = Math.min(count - 1, chooser.selected + 1)
                } else if (event.key === Qt.Key_Up) {
                    chooser.selected = Math.max(0, chooser.selected - 1)
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    chooser.choose(chooser.selected)
                } else {
                    return
                }
                event.accepted = true
            }

            delegate: Rectangle {
                id: row

                required property int index
                required property var modelData

                readonly property bool current: chooser.selected === index

                width: list.width
                height: chooser.rowHeight
                radius: CelestinaTheme.radiusSm
                color: row.current ? CelestinaTheme.surfaceSelected
                       : rowMouse.containsMouse ? CelestinaTheme.surfaceHover
                       : CelestinaTheme.controlFill
                border.width: row.current ? 1 : 0
                border.color: CelestinaTheme.borderStrong

                Behavior on color {
                    ColorAnimation { duration: CelestinaTheme.motionFast }
                }

                // Una miniatura proporcional a la salida real: dice de un
                // vistazo cuál es la ultrawide y cuál la vertical.
                Rectangle {
                    id: thumb
                    x: 16
                    anchors.verticalCenter: parent.verticalCenter
                    readonly property real ratio: row.modelData.height > 0
                            ? row.modelData.width / row.modelData.height : 1.6
                    height: 40
                    width: Math.round(height * ratio)
                    radius: CelestinaTheme.radiusXs
                    color: CelestinaTheme.canvas
                    border.width: 1
                    border.color: row.current ? CelestinaTheme.accent
                                              : CelestinaTheme.border
                }

                Text {
                    id: name
                    x: thumb.x + thumb.width + 16
                    y: row.height / 2 - height - 1
                    text: row.modelData.name
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    font.weight: CelestinaTheme.weightMedium
                }

                Text {
                    x: name.x
                    y: row.height / 2 + 1
                    width: row.width - x - 18
                    text: Math.round(row.modelData.width) + " × "
                          + Math.round(row.modelData.height)
                          + (row.modelData.devicePixelRatio > 1
                             ? "  ·  ×" + row.modelData.devicePixelRatio.toFixed(1) : "")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideRight
                }

                MouseArea {
                    id: rowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: chooser.selected = row.index
                    onDoubleClicked: chooser.choose(row.index)
                }
            }
        }

        Row {
            id: buttons
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            ChooserButton {
                text: "Cancelar"
                onClicked: chooser.cancel()
            }
            ChooserButton {
                text: "Compartir"
                primary: true
                onClicked: chooser.choose(chooser.selected)
            }
        }
    }

    component ChooserButton: Rectangle {
        id: button

        property string text: ""
        property bool primary: false
        signal clicked()

        implicitWidth: label.implicitWidth + 30
        width: implicitWidth
        height: 32
        radius: CelestinaTheme.radiusSm
        color: button.primary
               ? (buttonMouse.pressed ? Qt.darker(CelestinaTheme.accent, 1.18)
                  : buttonMouse.containsMouse ? Qt.darker(CelestinaTheme.accent, 1.08)
                  : CelestinaTheme.accent)
               : (buttonMouse.pressed ? CelestinaTheme.surfaceStrong
                  : buttonMouse.containsMouse ? CelestinaTheme.surfaceHover
                  : CelestinaTheme.controlFill)
        border.width: button.primary ? 0 : 1
        border.color: CelestinaTheme.border

        Behavior on color {
            ColorAnimation { duration: CelestinaTheme.motionFast }
        }

        Text {
            id: label
            anchors.centerIn: parent
            text: button.text
            color: button.primary ? CelestinaTheme.canvas : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontLabel
            font.weight: CelestinaTheme.weightMedium
        }

        MouseArea {
            id: buttonMouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: button.clicked()
        }
    }
}
