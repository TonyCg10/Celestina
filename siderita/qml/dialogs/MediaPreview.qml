import QtQuick
import org.celestina.siderita 1.0
// El tipo del crate compartido: la misma superficie que usa la ventana de
// Fluorita, en su propio espacio de nombres.
import org.celestina.fluorita.render 1.0

// El reproductor incrustado: la mitad de Fluorita que cabe en un gestor de
// archivos.
//
// Muestra sólo lo que este medio soporta de verdad y nada más — carátula si
// alguien ya la produjo, transporte si el tiempo significa algo aquí — y todo
// lo que dice viene confirmado por el motor. Un clic es una petición: el
// relleno no se mueve hasta que el motor lo confirma, y mientras tanto la marca
// de pendiente enseña adónde se pidió ir.
Item {
    id: preview

    required property SideritaPlayer player
    // La ruta, para pedirle la carátula al proveedor de miniaturas de Siderita:
    // sólo devuelve lo que ya está en la caché compartida, así que mirar una
    // carpeta nunca decodifica nada.
    required property string path

    readonly property bool playing: preview.player.state === "reproduciendo"
    readonly property bool failed: preview.player.state === "error"
    // Hay imagen en movimiento sólo cuando el motor ha dado un handle y la
    // superficie puede pintarlo; si no, se muestra la carátula.
    readonly property bool showsVideo: preview.player.kind === "vídeo"
        && preview.player.renderHandle !== 0

    // La imagen en movimiento ocupa el hueco entero; el transporte va debajo.
    MpvVideo {
        id: video

        anchors.fill: parent
        anchors.bottomMargin: CelestinaTheme.rowHeight
        visible: preview.showsVideo
        handle: preview.player.renderHandle

        // El orden importa en las dos direcciones: el motor no carga hasta que
        // existe el contexto, y la sesión no se suelta hasta que se ha ido.
        onContextCreated: preview.player.surfaceReady()
        onContextReleased: preview.player.surfaceReleased()

        Accessible.role: Accessible.Graphic
        Accessible.name: qsTr("Vídeo")
    }

    Column {
        anchors.centerIn: parent
        visible: !preview.showsVideo
        width: Math.min(parent.width, 420)
        spacing: CelestinaTheme.spaceMd

        // La carátula si existe; si no, el glifo del tipo. Nadie la genera aquí.
        Item {
            anchors.horizontalCenter: parent.horizontalCenter
            width: Math.min(preview.width, preview.height * 0.5, 220)
            height: width

            Image {
                id: cover

                anchors.fill: parent
                source: preview.path.length > 0
                    ? "image://thumb/" + encodeURIComponent(preview.path)
                    : ""
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: false
                visible: cover.status === Image.Ready
            }

            CelestinaIcon {
                anchors.centerIn: parent
                visible: !cover.visible
                width: parent.width * 0.45
                height: width
                sourceSize: Qt.size(width, height)
                name: "file-music"
                fallbackName: "file"
            }
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: preview.failed
                ? preview.player.errorText
                : preview.player.state === "abriendo"
                    ? qsTr("Abriendo…")
                    : preview.player.state === "terminado"
                        ? qsTr("Terminado")
                        : ""
            visible: text.length > 0
            color: preview.failed ? CelestinaTheme.danger : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

    }

    // El transporte vive fuera de la columna: con vídeo va bajo la imagen, y
    // con audio bajo la carátula, pero es el mismo control en ambos casos.
    Row {
        id: transportRow

        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceMd
        width: Math.min(parent.width, 420)
        spacing: CelestinaTheme.spaceMd
        visible: preview.player.timed && !preview.failed

        CelestinaIconButton {
            anchors.verticalCenter: parent.verticalCenter
            iconName: preview.playing ? "media-pause" : "media-play"
            helpText: preview.playing ? qsTr("Pausar") : qsTr("Reproducir")
            onClicked: preview.player.toggle()
        }

        CelestinaSlider {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - CelestinaTheme.controlHeight - 90
                   - CelestinaTheme.spaceMd * 2
            enabled: preview.player.durationMs > 0
            value: preview.player.positionMs
            to: preview.player.durationMs
            // Cinco segundos, como en Fluorita: el mismo gesto en las dos
            // superficies del mismo reproductor.
            step: 5000

            Accessible.name: qsTr("Posición")
            Accessible.description: qsTr("Flechas para avanzar o retroceder cinco segundos")

            onMoved: function(target) { preview.player.seek(Math.round(target)) }
        }

        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: 90
            horizontalAlignment: Text.AlignRight
            text: preview.clock(preview.player.positionMs)
                  + " / " + preview.clock(preview.player.durationMs)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
    }

    function clock(milliseconds) {
        const total = Math.max(0, Math.floor(milliseconds / 1000))
        const minutes = Math.floor(total / 60)
        const seconds = total % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }
}
