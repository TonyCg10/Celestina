pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

// Shared route reveal for the main browser and the portal picker. Call
// prepare() before replacing the route-bound model and play() afterwards, so
// the new contents settle in while the surrounding application stays fixed.
Item {
    id: root

    width: 0
    height: 0
    visible: false

    required property var navigationController
    required property bool ready
    property real progress: 1
    readonly property real offset: CelestinaTheme.reducedMotion
                                   ? 0
                                   : (1 - progress) * CelestinaTheme.spaceSm
    readonly property bool prepared: progress < 1 && !reveal.running

    function prepare() {
        reveal.stop()
        progress = CelestinaTheme.reducedMotion ? 1 : 0
    }

    function prepareIfReady() {
        if (ready)
            prepare()
    }

    function play() {
        if (CelestinaTheme.reducedMotion) {
            settle()
            return
        }
        reveal.restart()
    }

    function settle() {
        reveal.stop()
        progress = 1
    }

    function revealPreparedRoute() {
        if (!prepared)
            return
        Qt.callLater(function() {
            root.play()
        })
    }

    NumberAnimation {
        id: reveal
        target: root
        property: "progress"
        from: 0
        to: 1
        duration: CelestinaTheme.motionNormal
        easing.type: CelestinaTheme.easeStandard
    }

    Connections {
        target: root.navigationController
        function onCurrentPathChanged() { root.prepareIfReady() }
        function onTrashActiveChanged() { root.prepareIfReady() }
        function onRecentActiveChanged() { root.prepareIfReady() }
    }

    Connections {
        target: CelestinaTheme
        function onReducedMotionChanged() {
            if (CelestinaTheme.reducedMotion)
                root.settle()
        }
    }
}
