import QtQuick
import org.celestina.fluorita 1.0

// The rest of the selected folder, along the bottom of an open picture.
//
// It exists so moving to the next photograph does not mean leaving the item,
// going back to the grid and coming in again. It is hidden until the pointer
// comes near the bottom edge, because while you are looking at a photograph the
// photograph is the point and a permanent strip of thumbnails is in the way.
//
// Only a picture gets one. A video or a track gets `ContentArrows` instead:
// posters you cannot read at a glance over a playing film are furniture, not
// navigation.
//
// Keyboard reach is not the pointer's: the dock reveals itself whenever it or
// anything inside it holds focus, so Tab makes it appear rather than moving
// focus into something invisible.
Item {
    id: dock

    // Who knows what else is in the folder and where we are in it. The dock
    // draws that; it does not work it out a second time.
    required property ContentNavigator navigator

    signal activated(string key, string name, rect origin, string poster, string kind)

    readonly property int stripHeight: CelestinaTheme.spaceLg * 5
    // How close the pointer has to come. Deliberately taller than the strip: a
    // reveal that only triggers once you are already on top of the thing you
    // cannot see is not a reveal.
    readonly property int approachHeight: dock.stripHeight + CelestinaTheme.spaceLg * 3

    readonly property bool revealed: approach.hovered || strip.activeFocus

    // The band the pointer has to reach. It sits behind the strip and accepts
    // nothing, so it reveals without swallowing a click meant for the picture.
    HoverHandler {
        id: approach
    }

    height: dock.approachHeight
    // Nothing to navigate to but the item you are already on.
    visible: dock.navigator.navigable

    Item {
        id: strip

        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: dock.stripHeight
        activeFocusOnTab: true

        opacity: dock.revealed ? 1 : 0
        // Slides down out of the way rather than only fading, so a strip that
        // is on its way out cannot take a click on the picture behind it.
        y: dock.revealed ? 0 : dock.stripHeight
        enabled: dock.revealed

        // The reveal is a deliberate arrival, not a blink. It runs on the same
        // clock as the slide so the strip fades in *while* it rises instead of
        // being fully opaque before it has finished moving.
        Behavior on opacity {
            NumberAnimation {
                duration: strip.travel
                easing.type: CelestinaTheme.easeStandard
            }
        }

        Behavior on y {
            NumberAnimation {
                duration: strip.travel
                easing.type: CelestinaTheme.easeStandard
            }
        }

        readonly property int travel: CelestinaTheme.reducedMotion
            ? 0 : CelestinaTheme.motionSlow

        Accessible.role: Accessible.List
        Accessible.name: qsTr("Resto de la carpeta")

        ListView {
            id: filmstrip

            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceSm
            orientation: ListView.Horizontal
            clip: true
            spacing: CelestinaTheme.spaceSm
            boundsBehavior: Flickable.StopAtBounds
            model: dock.navigator.rows
            currentIndex: dock.navigator.index
            // Keeps what is open in view when the person arrives from the grid
            // or steps along the strip, and *travels* there rather than
            // teleporting: the strip slides the way the selection moved, so it
            // is obvious which direction you went.
            //
            // The empty highlight is what makes that possible. With none, the
            // view snaps the content to satisfy the range in a single frame;
            // with one, `highlightMoveDuration` governs the trip and the
            // content follows it. It draws nothing — the ring on the open frame
            // is the marker.
            highlight: Item { }
            highlightFollowsCurrentItem: true
            highlightMoveDuration: CelestinaTheme.reducedMotion
                ? 0 : CelestinaTheme.motionSlow
            highlightMoveVelocity: -1
            highlightRangeMode: ListView.ApplyRange
            preferredHighlightBegin: width / 3
            preferredHighlightEnd: width * 2 / 3

            delegate: Item {
                id: frame

                required property var modelData
                required property int index

                readonly property bool current: frame.modelData.key === dock.navigator.currentKey

                width: Math.round(filmstrip.height * 16 / 9)
                height: filmstrip.height

                Accessible.role: Accessible.ListItem
                Accessible.name: frame.modelData.name
                Accessible.focusable: true
                Accessible.onPressAction: dock.activated(frame.modelData.key, frame.modelData.name,
                                                    dock.originOf(frame), frame.modelData.thumbnail,
                                                    frame.modelData.kind)

                // The open item is ringed; the rest are the pictures alone,
                // floating over the content. A card each would put a strip of
                // furniture between the person and what they are looking at.
                Rectangle {
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: CelestinaTheme.clear
                    visible: frame.current
                    border.width: CelestinaTheme.borderFocus
                    border.color: CelestinaTheme.accent
                }

                Image {
                    id: poster

                    anchors.fill: parent
                    anchors.margins: CelestinaTheme.borderFocus
                    source: frame.modelData.thumbnail
                    visible: frame.modelData.thumbnail.length > 0
                        && poster.status === Image.Ready
                    asynchronous: true
                    autoTransform: true
                    fillMode: Image.PreserveAspectCrop
                    sourceSize.width: frame.width
                    sourceSize.height: frame.height
                }

                // No cached thumbnail: the kind, said with the theme's icon.
                CelestinaIcon {
                    anchors.centerIn: parent
                    visible: !poster.visible
                    width: CelestinaTheme.iconMd
                    height: width
                    sourceSize: Qt.size(width, height)
                    name: frame.modelData.kind === "video"
                        ? "file-video-camera"
                        : frame.modelData.kind === "audio" ? "file-music" : "file-image"
                    fallbackName: "file"
                }

                MouseArea {
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: dock.activated(frame.modelData.key, frame.modelData.name,
                                                    dock.originOf(frame), frame.modelData.thumbnail,
                                                    frame.modelData.kind)
                }
            }

            Keys.onReturnPressed: filmstrip.activateCurrent()
            Keys.onEnterPressed: filmstrip.activateCurrent()

            function activateCurrent() {
                const rows = dock.navigator.rows;
                if (filmstrip.currentIndex < 0 || filmstrip.currentIndex >= rows.length)
                    return;
                const frame = filmstrip.itemAtIndex(filmstrip.currentIndex);
                dock.activated(rows[filmstrip.currentIndex].key,
                               rows[filmstrip.currentIndex].name,
                               frame ? dock.originOf(frame) : Qt.rect(0, 0, 0, 0),
                               rows[filmstrip.currentIndex].thumbnail,
                               rows[filmstrip.currentIndex].kind);
            }
        }
    }

    // The same contract the grid uses: where the thing the person clicked is,
    // in the scene, so the window can grow the next item from it.
    function originOf(frame) {
        const point = frame.mapToItem(null, 0, 0);
        return Qt.rect(point.x, point.y, frame.width, frame.height);
    }
}
