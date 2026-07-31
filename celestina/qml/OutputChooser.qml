pragma ComponentBehavior: Bound

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
// Las salidas se muestran **en fila**, no en lista: un escritorio está puesto de
// izquierda a derecha, y elegir monitor es un gesto espacial, no de menú. Cada
// tarjeta guarda la proporción real de su pantalla, así que la ultrawide se
// reconoce por su forma antes que por su nombre.
//
// Lo lanza el shell (`celestina --pick-output`), que escribe el nombre elegido
// por stdout y sale con 0; cancelar sale con 1 sin escribir nada.
// ──────────────────────────────────────────────────────────────────────────────
Window {
    id: chooser

    required property bool reducedMotion
    // Live flattened list supplied by the C++ host. Each row has exactly `name`, `width`,
    // `height` and `devicePixelRatio`; QScreen objects do not expose standalone
    // width/height properties to QML, so the boundary is flattened explicitly.
    required property list<var> screens

    // `chosen` es el contrato con el host en C++: al fijarlo, el host imprime y
    // termina. Vacío + `cancelled` significa "el usuario no quiere compartir".
    property string chosen: ""
    property bool cancelled: false

    // La tarjeta más alta manda: las demás se alinean a su base.
    readonly property int tileHeight: 148
    readonly property int tileWidth: 236
    readonly property int tileSpacing: 16
    // La fila se lleva algo más de alto que las tarjetas: el fondo del elemento
    // marcado se dibuja hasta su borde, y pegado al recorte se veía cortado.
    readonly property int rowHeight: tileHeight + 12
    // El respiro entre la fila y los botones: es lo que la tarjeta reserva al
    // pedir su alto, y también lo que la fila respeta si el compositor le da
    // menos del que pidió.
    readonly property int rowActionsGap: 26
    // Cabecera + fila + un respiro real antes de los botones + el pie.
    readonly property int cardHeight: 84 + rowHeight + rowActionsGap + 64
    // Justo lo que ocupan las tarjetas más sus márgenes: sin hueco sobrante a la
    // derecha de la última pantalla. Los 12 de más son el aire que la fila se
    // reserva por dentro para que el borde de la tarjeta marcada no muera
    // pegado al recorte — la primera y la última se cortaban a lo largo.
    readonly property int cardWidth: Math.min(
            Math.max(380, screens.length * tileWidth
                          + Math.max(0, screens.length - 1) * tileSpacing + 68),
            Screen.width > 0 ? Screen.width - 120 : 1200)

    // La ventana pide 24 más que la tarjeta porque el panel se recorta a
    // `width - 20` para no tocar los bordes: pidiendo justo la anchura de la
    // tarjeta, el panel nacía 20 px más estrecho y la fila perdía ese ancho por
    // la derecha — la última pantalla marcada se cortaba.
    width: cardWidth + 24
    height: cardHeight + 24
    visible: true
    color: CelestinaTheme.clear
    flags: Qt.Dialog | Qt.FramelessWindowHint
    title: "Compartir pantalla"

    property int selected: 0
    // Preserve the selected output across a live QScreen snapshot. An index is
    // only a presentation detail: removing an earlier output must not silently
    // move the user's selection to a different monitor.
    property string selectedOutputName: ""

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = reducedMotion
        reconcileScreens()
    }

    onScreensChanged: reconcileScreens()

    function reconcileScreens() {
        if (screens.length === 0) {
            selected = 0
            selectedOutputName = ""
            return
        }

        let nextIndex = -1
        for (let i = 0; i < screens.length; ++i) {
            if (screens[i].name === selectedOutputName) {
                nextIndex = i
                break
            }
        }
        if (nextIndex < 0)
            nextIndex = Math.max(0, Math.min(selected, screens.length - 1))

        selectOutput(nextIndex)
    }

    function selectOutput(index) {
        if (index < 0 || index >= screens.length)
            return
        selected = index
        selectedOutputName = screens[index].name
        row.positionViewAtIndex(selected, ListView.Contain)
    }

    function choose(index) {
        if (index < 0 || index >= screens.length)
            return
        selectOutput(index)
        chosen = selectedOutputName
    }
    function cancel() {
        cancelled = true
    }

    // Tarjeta centrada, no ventana rellena: en un compositor de mosaico el
    // tamaño de la ventana lo decide él, y el diálogo debe verse igual.
    Rectangle {
        id: panel
        anchors.centerIn: parent
        width: Math.min(chooser.cardWidth, chooser.width - 20)
        height: Math.min(chooser.cardHeight, chooser.height - 20)
        radius: CelestinaTheme.radiusLg
        color: CelestinaTheme.surfaceStrong
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider

        Text {
            id: heading
            x: 24
            y: 20
            text: "Compartir pantalla"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        Text {
            id: subheading
            x: 24
            y: heading.y + heading.height + 2
            width: parent.width - 48
            text: chooser.screens.length > 1
                  ? "Elige qué salida verá la aplicación."
                  : "Se compartirá esta salida."
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideRight
        }

        // Las salidas, en fila y con su proporción real.
        ListView {
            id: row
            objectName: "outputRow"
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: subheading.bottom
            anchors.topMargin: 16
            anchors.leftMargin: 28
            anchors.rightMargin: 28
            // El alto que la fila pide, pero nunca más del que dejan los
            // botones. La ventana la decide el compositor y la tarjeta se
            // recorta a ella (arriba), así que con un alto impuesto la fila
            // seguía midiendo lo suyo y se montaba sobre el pie.
            height: Math.max(0, Math.min(chooser.rowHeight,
                                         actions.y - y - chooser.rowActionsGap))
            orientation: ListView.Horizontal
            spacing: chooser.tileSpacing
            // El recorte se lleva lo que toque su borde: estos márgenes son los
            // que dejan que el marcado se dibuje entero por los cuatro lados.
            leftMargin: 6
            rightMargin: 6
            topMargin: 6
            clip: true
            model: chooser.screens
            currentIndex: chooser.selected
            boundsBehavior: Flickable.StopAtBounds
            focus: true
            activeFocusOnTab: true
            KeyNavigation.tab: cancelButton
            KeyNavigation.backtab: shareButton
            Accessible.role: Accessible.List
            Accessible.name: "Pantallas disponibles"

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    chooser.cancel()
                } else if (event.key === Qt.Key_Right) {
                    chooser.selectOutput(Math.min(count - 1,
                                                  chooser.selected + 1))
                } else if (event.key === Qt.Key_Left) {
                    chooser.selectOutput(Math.max(0, chooser.selected - 1))
                } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                    chooser.choose(chooser.selected)
                } else {
                    return
                }
                event.accepted = true
            }

            delegate: Rectangle {
                id: tile

                required property int index
                required property var modelData

                readonly property bool current: chooser.selected === index

                width: chooser.tileWidth
                height: chooser.tileHeight
                radius: CelestinaTheme.radiusSm
                color: tile.current ? CelestinaTheme.surfaceSelected
                       : tileMouse.containsMouse ? CelestinaTheme.surfaceHover
                       : CelestinaTheme.controlFill
                border.width: tile.current ? CelestinaTheme.borderHairline : 0
                border.color: CelestinaTheme.dividerStrong

                Behavior on color {
                    ColorAnimation {
                        duration: CelestinaTheme.reducedMotion
                                  ? 0 : CelestinaTheme.motionFast
                    }
                }

                // La pantalla, a escala: 16:9 se ve ancha, una vertical se ve
                // alta. Se reconoce el monitor por su forma, no leyendo.
                Rectangle {
                    id: glass
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: 16
                    readonly property real ratio: tile.modelData.height > 0
                            ? tile.modelData.width / tile.modelData.height : 1.6
                    height: 52
                    width: Math.min(170, Math.round(height * ratio))
                    radius: CelestinaTheme.radiusSm
                    color: CelestinaTheme.canvas
                    border.width: CelestinaTheme.borderHairline
                    border.color: tile.current ? CelestinaTheme.accent
                                               : CelestinaTheme.divider

                    // El pie del monitor: pequeño, pero es lo que lo hace legible
                    // como pantalla y no como rectángulo.
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.bottom
                        width: Math.round(parent.width * 0.22)
                        height: 4
                        radius: height / 2
                        color: tile.current ? CelestinaTheme.accent
                                            : CelestinaTheme.divider
                    }
                }

                Text {
                    id: name
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: glass.y + glass.height + 16
                    width: tile.width - 20
                    horizontalAlignment: Text.AlignHCenter
                    text: tile.modelData.name
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    font.weight: CelestinaTheme.weightMedium
                    elide: Text.ElideRight
                }

                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    y: name.y + name.height + 1
                    width: tile.width - 20
                    horizontalAlignment: Text.AlignHCenter
                    text: Math.round(tile.modelData.width) + " × "
                          + Math.round(tile.modelData.height)
                          + (tile.modelData.devicePixelRatio > 1
                             ? "  ·  ×" + tile.modelData.devicePixelRatio.toFixed(1) : "")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideRight
                }

                MouseArea {
                    id: tileMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: chooser.selectOutput(tile.index)
                    onDoubleClicked: chooser.choose(tile.index)
                }

                Accessible.role: Accessible.ListItem
                Accessible.name: tile.modelData.name
                Accessible.selected: tile.current
                Accessible.onPressAction: chooser.choose(tile.index)
            }
        }

        Row {
            id: actions
            objectName: "chooserActions"
            anchors.right: parent.right
            anchors.rightMargin: 28
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 20
            spacing: 12

            CelestinaButton {
                id: cancelButton

                text: "Cancelar"
                KeyNavigation.tab: shareButton
                KeyNavigation.backtab: row
                onClicked: chooser.cancel()
            }
            CelestinaButton {
                id: shareButton

                text: "Compartir"
                role: CelestinaButton.Primary
                enabled: chooser.screens.length > 0
                KeyNavigation.tab: row
                KeyNavigation.backtab: cancelButton
                onClicked: chooser.choose(chooser.selected)
            }
        }
    }

}
