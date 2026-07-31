import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// Fluorita's window, in two modes and no more: handed a file it is a player,
// launched bare it is a library. Activating something from the library is the
// same path as being handed it on the command line, so there is one way to
// start playing rather than two that can disagree.
ApplicationWindow {
    id: window

    required property bool reducedMotion
    // The item named on the command line, already reduced to a display label
    // by the Rust side. Empty means Fluorita was launched with no argument.
    required property string requestedLabel
    // Its classified kind, decided from the name alone — no decoder ran.
    required property string requestedKind
    // The real path, byte-exact, for the engine. Never rebuilt from the label.
    required property string requestedPath

    width: 960
    height: 640
    minimumWidth: 420
    minimumHeight: 320
    visible: true
    color: CelestinaTheme.canvas
    title: window.openLabel.length > 0
        ? qsTr("Fluorita — %1").arg(window.openLabel)
        : qsTr("Fluorita")

    // Abrir un item: el reproductor toma la ventana. La etiqueta es sólo para
    // mostrar; la ruta es la que va al motor.
    function open(path) {
        window.openPath = path;
        window.openLabel = path.substring(path.lastIndexOf("/") + 1);
        mediaPlayer.open(path);
    }

    function backToLibrary() {
        mediaPlayer.close();
        window.openPath = "";
        window.openLabel = "";
    }

    // Empty until something is activated: the library is what a bare launch
    // shows, and the player takes over the window when there is an item.
    property string openPath: window.requestedPath
    property string openLabel: window.requestedLabel
    readonly property bool playing: window.openPath.length > 0

    FluoritaPlayer {
        id: mediaPlayer
    }

    FluoritaLibrary {
        id: mediaLibrary
    }

    CelestinaBackdrop {
        anchors.fill: parent
        visible: !window.playing || !playerSurface.showsPicture
    }

    LibraryView {
        anchors.fill: parent
        visible: !window.playing
        library: mediaLibrary
        // Activating an item is exactly what the command line does, so it goes
        // through the same door.
        onActivated: function(path) { window.open(path) }
    }

    PlayerSurface {
        id: playerSurface

        anchors.fill: parent
        visible: window.playing
        player: mediaPlayer
        label: window.openLabel
    }

    Shortcut {
        sequence: "Space"
        enabled: window.playing
        onActivated: mediaPlayer.toggle()
    }
    // Generar las miniaturas que faltan, sin depender del ratón ni del orden
    // de tabulación.
    Shortcut {
        sequence: "Ctrl+G"
        enabled: !window.playing && mediaLibrary.artworkPending > 0
            && mediaLibrary.artworkState === "parada"
        onActivated: mediaLibrary.generateArtwork()
    }
    Shortcut {
        sequence: "Ctrl+Shift+G"
        enabled: !window.playing && mediaLibrary.artworkState === "generando"
        onActivated: mediaLibrary.cancelArtwork()
    }

    // Volver a la biblioteca: cierra la sesión y devuelve la ventana a las
    // rejillas, sin salir de la aplicación.
    Shortcut {
        sequence: "Escape"
        enabled: window.playing
        onActivated: window.backToLibrary()
    }
    Shortcut {
        sequences: [StandardKey.Close, StandardKey.Quit]
        onActivated: window.close()
    }

    // Closing while a session is open would drop the backend under the surface
    // still rendering from it: the player clears the handle first and the
    // window leaves once the surface has confirmed.
    property bool closeAuthorised: false

    onClosing: function(close) {
        if (window.closeAuthorised || mediaPlayer.renderHandle === 0) {
            close.accepted = true
            return
        }
        close.accepted = false
        window.closeAuthorised = true
        mediaPlayer.close()
        Qt.callLater(window.close)
    }

    Component.onCompleted: {
        CelestinaTheme.reducedMotion = window.reducedMotion
        if (window.requestedPath.length > 0) {
            mediaPlayer.open(window.requestedPath)
        } else {
            // Sin argumento: la biblioteca. El escaneo corre en el worker del
            // motor, así que esta llamada vuelve de inmediato.
            mediaLibrary.scan()
        }
    }
}
