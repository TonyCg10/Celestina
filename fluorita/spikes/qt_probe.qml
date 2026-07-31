// Sonda mínima de Qt Multimedia para el spike F2.
//
// No es código de producto: sólo abre el fixture que le pasa el harness por
// entorno, reporta cuándo el backend confirma metadata y primer estado de
// reproducción, y se cierra sola. Corre bajo `QT_QPA_PLATFORM=offscreen`, así
// que mide decodificación y coste de proceso, nunca presentación real.

import QtQuick
import QtMultimedia

Item {
    id: root

    readonly property string fixture: Qt.application.arguments.length > 1
        ? Qt.application.arguments[Qt.application.arguments.length - 1]
        : ""
    readonly property int playSeconds: 10

    property double startedAt: Date.now()
    property bool reportedFirstFrame: false

    function stamp(event) {
        console.log("probe", event, (Date.now() - root.startedAt) + "ms");
    }

    VideoOutput {
        id: output
        anchors.fill: parent
    }

    MediaPlayer {
        id: player

        source: root.fixture ? "file://" + root.fixture : ""
        videoOutput: output

        onMediaStatusChanged: {
            if (mediaStatus === MediaPlayer.LoadedMedia) {
                root.stamp("loaded duration=" + duration);
                player.play();
            } else if (mediaStatus === MediaPlayer.InvalidMedia) {
                root.stamp("invalid");
                Qt.exit(3);
            }
        }

        onPositionChanged: {
            if (!root.reportedFirstFrame && position > 0) {
                root.reportedFirstFrame = true;
                root.stamp("first-position");
            }
        }

        onErrorOccurred: function(error, message) {
            root.stamp("error " + message);
            Qt.exit(4);
        }
    }

    Timer {
        interval: root.playSeconds * 1000
        running: true
        onTriggered: {
            root.stamp("stop position=" + player.position);
            player.stop();
            Qt.exit(root.reportedFirstFrame ? 0 : 5);
        }
    }

    Component.onCompleted: root.stamp("start fixture=" + root.fixture)
}
