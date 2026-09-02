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
    // `renderHandle` se pone a distinto de cero en cuanto el motor crea la
    // instancia de mpv — que siempre se crea, sea cual sea el contenido del
    // archivo — mucho antes de intentar abrirlo o decodificarlo. Fiarse sólo
    // del handle dejaba una superficie negra para siempre con un archivo que
    // mpv nunca podía abrir: el texto de estado/error de debajo quedaba
    // tapado por un vídeo "activo" que jamás iba a pintar un fotograma. La
    // imagen en movimiento sólo se muestra cuando el motor ya confirmó
    // reproducción real.
    readonly property bool confirmedPlaying: preview.player.state === "reproduciendo"
        || preview.player.state === "pausado"
        || preview.player.state === "terminado"
    readonly property bool showsVideo: preview.player.kind === "vídeo"
        && preview.player.renderHandle !== 0
        && preview.confirmedPlaying

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
                    ? "image://thumb/" + preview.path
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
            width: parent.width - CelestinaTheme.controlHeight - 90 - volumeGroup.width
                   - CelestinaTheme.spaceMd * 3
            // The shared track is 16 px tall by default; a seek bar is grabbed
            // in passing, so it gets the 30 px floor. The track stays centred.
            height: CelestinaTheme.controlHeightXs
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

        Row {
            id: volumeGroup

            // El icono recuerda a qué nivel volver al desmutear. Sólo se
            // guarda mientras suena algo: guardar un 0 dejaría el botón sin
            // adónde volver.
            property int rememberedPercent: 100
            readonly property bool muted: preview.player.volumePercent <= 0

            anchors.verticalCenter: parent.verticalCenter
            width: CelestinaTheme.controlHeightXs + CelestinaTheme.spaceSm + 64
            spacing: CelestinaTheme.spaceSm

            Connections {
                target: preview.player
                function onVolumePercentChanged() {
                    if (preview.player.volumePercent > 0)
                        volumeGroup.rememberedPercent = preview.player.volumePercent
                }
            }

            CelestinaIconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: volumeGroup.muted ? "media-volume-muted" : "media-volume"
                helpText: volumeGroup.muted ? qsTr("Activar sonido") : qsTr("Silenciar")
                onClicked: volumeGroup.muted
                    ? preview.player.setVolume(
                          volumeGroup.rememberedPercent > 0 ? volumeGroup.rememberedPercent : 100)
                    : preview.player.setVolume(0)
            }

            CelestinaSlider {
                anchors.verticalCenter: parent.verticalCenter
                width: 64
                height: CelestinaTheme.controlHeightXs
                value: preview.player.volumePercent
                to: 100
                step: 5

                Accessible.name: qsTr("Volumen")
                Accessible.description: qsTr("Flechas para subir o bajar el volumen")

                onMoved: function(target) { preview.player.setVolume(Math.round(target)) }
            }
        }
    }

    function clock(milliseconds) {
        const total = Math.max(0, Math.floor(milliseconds / 1000))
        const minutes = Math.floor(total / 60)
        const seconds = total % 60
        return minutes + ":" + (seconds < 10 ? "0" : "") + seconds
    }
}
