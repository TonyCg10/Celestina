import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0
// The render surface is hand-written C++ and therefore its own namespace.
import org.celestina.fluorita.render 1.0

// The picture and the transport for one item.
//
// It owns no truth: everything it shows comes from the player, whose properties
// only move when the engine confirms something. A pending click is shown as
// pending rather than as a state that has not happened.
Item {
    id: surface

    required property FluoritaPlayer player
    // Empty until something is open; shown when there is no picture to show.
    required property string label

    // Three surfaces, one at a time: moving picture, still, or a plain label.
    readonly property bool showsVideo: surface.player.hasVideo
        && surface.player.renderHandle !== 0
    readonly property bool showsImage: surface.player.imageSource.length > 0
    readonly property bool showsPicture: surface.showsVideo || surface.showsImage

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Reproductor")

    MpvVideo {
        id: video

        anchors.fill: parent
        visible: surface.showsVideo
        // The one value the surface needs from the engine: a zero handle means
        // there is nothing to render, including while a session is closing.
        handle: surface.player.renderHandle

        onContextCreated: surface.player.surfaceReady()
        onContextReleased: surface.player.surfaceReleased()
    }

    ImageView {
        anchors.fill: parent
        anchors.bottomMargin: CelestinaTheme.spaceLg * 3
        visible: surface.showsImage
        source: surface.player.imageSource
    }

    // Audio, or a video whose surface is not up: say what is playing instead of
    // showing an empty rectangle that looks broken.
    ColumnLayout {
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.spaceLg * 2, 520)
        spacing: CelestinaTheme.spaceSm
        visible: !surface.showsPicture

        CelestinaSectionLabel {
            Layout.fillWidth: true
            text: surface.player.state
        }

        Text {
            Layout.fillWidth: true
            text: surface.label
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideMiddle
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Text {
            Layout.fillWidth: true
            visible: surface.player.errorMessage.length > 0
            text: surface.player.errorMessage
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
    }

    PlayerTransport {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: CelestinaTheme.spaceLg
        player: surface.player
    }
}
