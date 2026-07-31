import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// La biblioteca: Galería y Música, las dos secciones de primera clase del
// producto. Nada aquí decodifica: el escaneo corre en el worker del motor y las
// miniaturas salen del caché compartido si ya existen.
Item {
    id: view

    required property FluoritaLibrary library
    signal activated(string path)

    // 0 = Galería, 1 = Música. Vive aquí porque las dos secciones y la pila
    // comparten la misma verdad.
    property int section: 0

    ColumnLayout {
        anchors.fill: parent
        anchors.margins: CelestinaTheme.spaceLg
        spacing: CelestinaTheme.spaceMd

        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceMd

            // Dos secciones fijas, hechas con el botón compartido: el
            // contrato prohíbe reconstruir controles Qt, y un TabBar crudo
            // sería exactamente eso. `Selected` es el rol que ya expresa
            // "esta es la sección activa".
            Row {
                spacing: CelestinaTheme.spaceSm

                CelestinaButton {
                    activeFocusOnTab: true
                    text: qsTr("Galería")
                    role: view.section === 0
                        ? CelestinaButton.Selected
                        : CelestinaButton.Ghost
                    enabled: view.library.imageCount + view.library.videoCount > 0
                    Accessible.name: text
                    Accessible.description: qsTr("Imágenes y vídeos de tu biblioteca")
                    onClicked: view.section = 0
                }

                CelestinaButton {
                    activeFocusOnTab: true
                    text: qsTr("Música")
                    role: view.section === 1
                        ? CelestinaButton.Selected
                        : CelestinaButton.Ghost
                    enabled: view.library.trackCount > 0
                    Accessible.name: text
                    Accessible.description: qsTr("Pistas por artista y álbum")
                    onClicked: view.section = 1
                }
            }

            Item { Layout.fillWidth: true }

            // Generar miniaturas es lo único aquí que arranca el motor, así
            // que es una decisión del usuario y no un efecto secundario de
            // abrir la ventana. Desaparece cuando no hay nada que generar.
            CelestinaButton {
                activeFocusOnTab: visible
                visible: view.library.artworkPending > 0
                    || view.library.artworkState !== "parada"
                text: view.library.artworkState === "parada"
                    ? qsTr("Generar %1 miniaturas").arg(view.library.artworkPending)
                    : view.library.artworkState === "cancelando"
                        ? qsTr("Cancelando…")
                        : qsTr("Generando %1 de %2 — cancelar")
                            .arg(view.library.artworkDone)
                            .arg(view.library.artworkTotal)
                role: view.library.artworkState === "generando"
                    ? CelestinaButton.Selected
                    : CelestinaButton.Tonal
                enabled: view.library.artworkState !== "cancelando"
                Accessible.name: text
                Accessible.description: qsTr(
                    "Extrae el fotograma o la carátula que falta en la caché compartida. Ctrl+G")
                onClicked: view.library.artworkState === "generando"
                    ? view.library.cancelArtwork()
                    : view.library.generateArtwork()
            }

            Text {
                text: view.library.summary
                color: view.library.truncated
                    ? CelestinaTheme.warning
                    : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                elide: Text.ElideRight
                Layout.maximumWidth: parent.width / 2
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }

        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: view.section

            GalleryGrid {
                library: view.library
                onActivated: function(path) { view.activated(path) }
            }

            MusicList {
                library: view.library
                onActivated: function(path) { view.activated(path) }
            }
        }
    }

    // Estados honestos: explorando, vacía, o un fallo con su motivo. Ninguno
    // finge una cuadrícula que no existe.
    Text {
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.spaceLg * 2, 460)
        visible: view.library.state !== "lista"
            || (view.library.imageCount + view.library.videoCount
                + view.library.trackCount) === 0
        text: view.library.state === "explorando"
            ? qsTr("Explorando tus carpetas…")
            : view.library.summary
        color: view.library.state === "error"
            ? CelestinaTheme.danger
            : CelestinaTheme.textFaint
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
