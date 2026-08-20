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
    // The open item's key, passed through to the transport for the verbs that
    // act on the file rather than on the session.
    required property string itemKey
    // Empty until something is open; shown when there is no picture to show.
    required property string label
    // The item's own artwork, used to light the space the picture does not
    // fill. Empty when nothing is cached for it, and then the canvas stays as
    // it is rather than being lit by a colour nobody measured.
    required property string ambientSource

    // Three surfaces, one at a time: moving picture, still, or a plain label.
    //
    // `renderHandle` turns nonzero as soon as the backend creates an mpv
    // instance — which always succeeds, whatever the file contains — well
    // before it has tried to open or decode anything. Gating on the handle
    // alone showed a black surface forever for a file mpv can never open: the
    // state/error label underneath was hidden by an "active" video item that
    // was never going to render a frame. The picture only shows once the
    // engine has confirmed real playback, so a bad file falls through to the
    // label below instead of a silent black screen.
    readonly property bool confirmedPlaying: surface.player.state === "reproduciendo"
        || surface.player.state === "pausado"
        || surface.player.state === "terminado"
    readonly property bool showsVideo: surface.player.hasVideo
        && surface.player.renderHandle !== 0
        && surface.confirmedPlaying
    readonly property bool showsImage: surface.player.imageSource.length > 0
    readonly property bool showsPicture: surface.showsVideo || surface.showsImage
    // What is actually on screen, as opposed to what has been asked for. A
    // still is only present once the toolkit decoded it; a video only once the
    // engine confirmed playback and the render surface exists. Anything
    // handing over to this surface must wait for *this*, not for `showsPicture`.
    readonly property bool picturePresented: surface.showsVideo
        || (surface.showsImage && still.presented)

    // The render context this surface owned is gone and the backend instance
    // may be destroyed. Whoever is dismantling the window or the frame waits
    // for this rather than for a timer.
    signal released()

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Reproductor")

    // Under everything, including the still and the video: it is light, not
    // content, and nothing here reads it or reacts to it.
    AmbientLight {
        id: ambient

        anchors.fill: parent
        source: surface.ambientSource
    }

    // Sized to the film rather than filled, whenever the artwork tells us what
    // shape the film is.
    //
    // Filling meant mpv drew its own letterbox — black — across the whole
    // surface, and black bands painted over the ambient light are exactly the
    // hole the light exists to remove. Letterboxing by geometry leaves those
    // bands unpainted, so what shows through them is the light.
    //
    // With no artwork there is no shape to trust and no light to reveal, so it
    // falls back to filling, which is what it always did.
    MpvVideo {
        id: video

        readonly property bool shaped: ambient.lit && ambient.contentAspect > 0
        readonly property real fitted: Math.min(surface.width / ambient.contentAspect,
                                               surface.height)

        anchors.centerIn: video.shaped ? surface : undefined
        anchors.fill: video.shaped ? undefined : parent
        width: video.shaped ? Math.round(video.fitted * ambient.contentAspect) : 0
        height: video.shaped ? Math.round(video.fitted) : 0
        // Stays in the scene graph for as long as its renderer holds a context,
        // and disappears from view instead by going transparent. Hiding it the
        // moment playback stopped being confirmed removed the very item whose
        // renderer has to free that context — on the render thread, where a
        // hidden item is never synchronized again — and left the mpv core to be
        // destroyed underneath it.
        visible: surface.showsVideo || video.rendererLive
        // Transparent rather than absent: the state and error labels below must
        // not be covered by a rectangle that is never going to show a frame.
        opacity: surface.showsVideo ? 1 : 0
        // The one value the surface needs from the engine: a zero handle means
        // there is nothing to render, including while a session is closing.
        handle: surface.player.renderHandle

        onContextCreated: surface.player.surfaceReady()
        onContextReleased: {
            surface.player.surfaceReleased()
            surface.released()
        }
        onContextFailed: surface.player.surfaceFailed()
    }

    // The still's zoom, published so the window can put a magnifier beside its
    // other actions without reaching into this surface's children.
    readonly property ZoomController imageZoom: still.zoom

    ImageView {
        id: still

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

    // What the picture is actually doing, when someone asks.
    //
    // Off by default and never in the way: judder is rare enough that a
    // permanent read-out would be furniture, and specific enough that the
    // person who sees it needs numbers within seconds of seeing it.
    Rectangle {
        id: pacing

        anchors.top: parent.top
        anchors.left: parent.left
        anchors.margins: CelestinaTheme.spaceLg
        width: pacingLine.implicitWidth + CelestinaTheme.spaceMd * 2
        height: pacingColumn.implicitHeight + CelestinaTheme.spaceSm * 2
        radius: CelestinaTheme.radiusMd
        color: CelestinaTheme.scrim
        visible: surface.player.capturingPacing

        Column {
            id: pacingColumn

            anchors.centerIn: parent
            spacing: CelestinaTheme.spaceXs

            Text {
                id: pacingLine

                text: surface.player.pacingLine
                color: surface.player.pacingVerdict === "dropping"
                    ? CelestinaTheme.danger
                    : surface.player.pacingVerdict === "delayed"
                        ? CelestinaTheme.warning
                        : CelestinaTheme.text
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }

            Text {
                visible: surface.player.pacingReport.length > 0
                text: surface.player.pacingReport
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.monoFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                elide: Text.ElideMiddle
                width: pacingLine.width
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }
    }

    PlayerTransport {
        id: playerTransport

        itemKey: surface.itemKey

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.margins: CelestinaTheme.spaceLg
        player: surface.player
    }

    // Without this, arrow-key seeking silently went nowhere until the seek
    // bar was clicked once: nothing ever gives it focus on its own, so
    // whatever the library last focused (a grid or list cell, `focus: true`
    // there) kept it even after the surface it belongs to was hidden.
    onVisibleChanged: if (surface.visible) playerTransport.focusSeek()
    Component.onCompleted: if (surface.visible) playerTransport.focusSeek()
}
