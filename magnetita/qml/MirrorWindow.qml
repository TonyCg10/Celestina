import QtQuick
import QtQuick.Window
import org.celestina.magnetita 1.0
import org.celestina.fluorita.render 1.0

// The phone's screen as a window of this application: the picture the
// daemon streams, decoded here, and the pointer, wheel and keys sent back
// as the phone's touches, swipes and keys. The window opens at the
// picture's own size and takes whatever size the compositor then gives
// it, a tile included: the picture fills it edge to edge, scaled, never
// letterboxed, and touches map through the whole window.
Window {
    id: mirror

    required property var view

    readonly property real aspect: view.pictureWidth > 0 && view.pictureHeight > 0
                                   ? view.pictureWidth / view.pictureHeight : 9 / 19.5

    visible: view.streaming
    // Its own toplevel, not a dialog of the main window: a transient window
    // is what a compositor floats, and this one belongs in the layout.
    transientParent: null
    title: qsTr("Espejo")
    color: CelestinaTheme.canvas
    width: view.pictureWidth > 0 ? view.pictureWidth : 460
    height: view.pictureHeight > 0 ? view.pictureHeight : 998
    minimumWidth: 120
    minimumHeight: 120

    onClosing: function(close) {
        close.accepted = true
        view.closeRequested()
    }

    // The picture fills the window: the engine scales it to the item, so
    // a touch maps over the whole window.
    readonly property real fitWidth: width
    readonly property real fitHeight: height
    readonly property real fitX: 0
    readonly property real fitY: 0

    // The window keeps the picture's aspect: whatever height the compositor
    // gives it (a tile on this output, another on the next), it asks for
    // the width that fits, settled a moment after the last resize.
    onWidthChanged: fitTimer.restart()
    onHeightChanged: fitTimer.restart()
    Timer {
        id: fitTimer
        interval: 120
        onTriggered: mirror.view.fit(mirror.width, mirror.height, Math.round(mirror.height * mirror.aspect))
    }

    // Diagnostics for the daemon's journal while the window is being brought
    // up: whether Qt paints this window at all, and whether the surface
    // holds a render context.
    property int swaps: 0
    onFrameSwapped: swaps++
    Timer {
        interval: 5000
        running: mirror.visible
        repeat: true
        onTriggered: {
            console.warn("magnetita: mirror window: swaps=" + mirror.swaps
                         + " live=" + video.rendererLive + " visible=" + video.visible
                         + " handle=" + mirror.view.renderHandle
                         + " size=" + mirror.width + "x" + mirror.height)
            mirror.swaps = 0
        }
    }

    MpvVideo {
        id: video
        anchors.fill: parent
        visible: mirror.view.renderHandle !== 0
        handle: mirror.view.renderHandle
        onContextFailed: console.warn("magnetita: mirror window: context failed")

        // The order matters both ways: the engine loads only once the context
        // exists, and the instance goes only once the context is gone.
        onContextCreated: mirror.view.surfaceReady()
        onContextReleased: mirror.view.surfaceReleased()

        Accessible.role: Accessible.Graphic
        Accessible.name: qsTr("Pantalla del móvil")
    }

    Text {
        anchors.centerIn: parent
        visible: mirror.view.error.length > 0
        text: mirror.view.error
        color: CelestinaTheme.text
        font.pixelSize: CelestinaTheme.fontRowTitle
    }

    // The finger. Picture pixels: the pointer through the fitted rectangle.
    MouseArea {
        id: finger
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton | Qt.BackButton | Qt.ForwardButton
        hoverEnabled: false
        cursorShape: Qt.ArrowCursor

        function picture(mouse) {
            return {
                x: Math.round((mouse.x - mirror.fitX) * mirror.view.pictureWidth / Math.max(1, mirror.fitWidth)),
                y: Math.round((mouse.y - mirror.fitY) * mirror.view.pictureHeight / Math.max(1, mirror.fitHeight))
            }
        }

        onPressed: function(mouse) {
            mirror.requestActivate()
            keys.forceActiveFocus()
            if (mouse.button === Qt.LeftButton) {
                const p = picture(mouse)
                mirror.view.touch(0, p.x, p.y)
            } else if (mouse.button === Qt.RightButton || mouse.button === Qt.BackButton) {
                mirror.view.global("Back")
            } else if (mouse.button === Qt.MiddleButton) {
                mirror.view.global("Home")
            } else if (mouse.button === Qt.ForwardButton) {
                mirror.view.global("Recents")
            }
        }
        onPositionChanged: function(mouse) {
            if (mouse.buttons & Qt.LeftButton) {
                const p = picture(mouse)
                mirror.view.touch(1, p.x, p.y)
            }
        }
        onReleased: function(mouse) {
            if (mouse.button === Qt.LeftButton) {
                const p = picture(mouse)
                mirror.view.touch(2, p.x, p.y)
            }
        }
        onCanceled: mirror.view.touch(2, 0, 0)
        onWheel: function(wheel) {
            const p = picture(wheel)
            mirror.view.wheel(wheel.angleDelta.y < 0 ? 1 : -1, p.x, p.y)
            wheel.accepted = true
        }
    }

    // The keyboard: F1, F2, F3 are Back, Home, Recents; the rest go as keys.
    Item {
        id: keys
        anchors.fill: parent
        focus: true
        Keys.onPressed: function(event) {
            if (event.isAutoRepeat)
                return
            if (event.key === Qt.Key_F1) mirror.view.global("Back")
            else if (event.key === Qt.Key_F2) mirror.view.global("Home")
            else if (event.key === Qt.Key_F3) mirror.view.global("Recents")
            else mirror.view.key(event.key, true)
            event.accepted = true
        }
        Keys.onReleased: function(event) {
            if (event.isAutoRepeat)
                return
            if (event.key !== Qt.Key_F1 && event.key !== Qt.Key_F2 && event.key !== Qt.Key_F3)
                mirror.view.key(event.key, false)
            event.accepted = true
        }
    }
}
