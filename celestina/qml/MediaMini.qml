// What the desktop is playing, as the session's own player reports it.
//
// It shows what the provider last confirmed and nothing between updates: the
// progress line moves when a new position arrives, not on a timer of its own,
// because a panel that animates ahead of its source is guessing. With no
// player the provider withdraws its value and this disappears rather than
// keeping the last thing that ever played.
import CelestinaStyle
import QtQuick

Item {
    id: root

    // The `media` provider's fields, or `undefined` when nothing is playing.
    // `var` is necessary because QML has no typed map.
    required property var reading
    // A click is a request: the host asks the player, and the next value says
    // what the player did.
    signal toggleRequested()

    readonly property bool hasPlayer: reading !== undefined
                                      && reading.nowPlaying !== undefined
                                      && reading.nowPlaying.length > 0
    readonly property bool finite: hasPlayer && reading.progress === "finite"
                                   && reading.lengthMs > 0
    // Present only when the provider could check it: a local file, bounded in
    // size, that starts like an image. Anything else shows no cover at all.
    readonly property string artPath: hasPlayer && reading.artPath !== undefined
                                      ? reading.artPath : ""

    // Measure the title independently from the elided Text's assigned width.
    // The panel starts this item at zero width; asking that Text for its
    // implicit width creates a zero-width sizing cycle on the live surface.
    implicitWidth: hasPlayer
                   ? Math.min(220, titleMetrics.advanceWidth) + cover.width + (cover.visible ? CelestinaTheme.spaceSm : 0)
                   : 0
    implicitHeight: 26
    visible: hasPlayer
    Accessible.role: Accessible.Button
    Accessible.name: hasPlayer ? reading.nowPlaying : ""
    Accessible.description: hasPlayer && reading.playing
                            ? qsTr("Sonando; pausa la reproducción")
                            : qsTr("En pausa; reanuda la reproducción")
    Accessible.onPressAction: root.toggleRequested()

    Image {
        id: cover

        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: visible ? 20 : 0
        height: 20
        visible: root.artPath.length > 0 && status === Image.Ready
        source: root.artPath.length > 0 ? "file://" + root.artPath : ""
        // The decode is bounded to what is drawn: a checked signature says a
        // file starts like an image, not that it is a sane size to expand.
        sourceSize.width: 40
        sourceSize.height: 40
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
        smooth: true
    }

    TextMetrics {
        id: titleMetrics

        text: root.hasPlayer ? root.reading.nowPlaying : ""
        font: label.font
    }

    Text {
        id: label

        anchors.left: cover.visible ? cover.right : parent.left
        anchors.leftMargin: cover.visible ? CelestinaTheme.spaceSm : 0
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.topMargin: 2
        text: root.hasPlayer ? root.reading.nowPlaying : ""
        // A paused player is still the one the panel is showing; it just is not
        // making a sound, and the ink says so rather than a second icon.
        color: root.hasPlayer && root.reading.playing
               ? CelestinaTheme.text : CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontCaption
        elide: Text.ElideRight
    }

    // Only drawn for media that has a real length. A live stream has a position
    // with nothing to measure it against, so it gets no bar at all.
    Rectangle {
        id: track

        anchors.left: label.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: 4
        height: CelestinaTheme.borderHairline * 2
        radius: height / 2
        visible: root.finite
        color: CelestinaTheme.surfaceSelected

        Rectangle {
            // Guarded in the expression, not only by `visible`: a binding is
            // evaluated whether or not what it draws is on screen, and there is
            // no position to read when no player is running.
            width: root.finite
                   ? parent.width * Math.min(1, root.reading.positionMs / root.reading.lengthMs)
                   : 0
            height: parent.height
            radius: parent.radius
            color: CelestinaTheme.accentLink
        }

    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggleRequested()
    }

}
