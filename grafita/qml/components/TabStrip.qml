// The tab delegates reach the strip's `root` and `tabRepeater` ids, which a
// delegate may only do under bound component behaviour; each declares the
// `index` it takes from the model.
pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.grafita 1.0

// The open documents, one row.
//
// Always there, including with a single document: hiding it meant the strip
// appeared and shifted the whole editor down the moment a second file arrived,
// and it left the "new tab" button nowhere to be found until you already had
// two. A steady row is worth the few pixels.
//
// Tabs are positioned by hand rather than with a Row, because dragging one to
// reorder means one delegate's `x` must escape the layout while it is held and
// every other delegate must still know where it belongs. Each delegate
// measures its own label (a private `TextMetrics`, so nothing mutates a
// property another tab's layout depends on) and exposes the resulting width as
// an ordinary property; `xFor(index)` only ever *reads* those already-settled
// widths, never recomputes one — the read/write split is what keeps this from
// becoming a binding loop.
//
// The row lives in a horizontal Flickable. Enough documents and the tabs are
// wider than the window; without a viewport the later ones — and the "+"
// button after them — sat past the window edge with no way to reach them.
// The button stays outside the viewport, pinned to the right edge once the
// tabs overflow, so it is always the same click away.
Item {
    id: root

    required property var tabs          // ListModel of open documents
    required property int current
    // Reads a tab's live state from the window, which owns the sessions.
    required property var titleFor      // function(index) -> string
    required property var dirtyFor      // function(index) -> bool

    // Emitted when a drag has settled on a new position. The model mutation
    // lives in the window, not here, because moving the *current* tab also
    // means re-finding which index now holds it — state this component does
    // not own.
    signal selected(int index)
    signal closeRequested(int index)
    signal newRequested
    signal reorderRequested(int from, int to)

    // A revision the window bumps so the delegates re-read titles that live
    // outside the model — a document's name arrives after its file opens.
    property int revision: 0

    implicitHeight: viewport.height + CelestinaTheme.spaceSm

    Rectangle {
        anchors.fill: parent
        color: CelestinaTheme.surface
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }

    // The close button is the Compact icon button, so the room a tab reserves
    // for it is exactly that button's height, not the Regular one's.
    readonly property real closeButtonWidth: CelestinaTheme.controlHeightXs

    // The already-settled width of tab `index`, or 0 while it has not been
    // created yet. Never computes anything itself — only reads what that
    // delegate already measured for itself.
    function widthFor(index) {
        const item = tabRepeater.itemAt(index)
        return item ? item.width : 0
    }

    // The x position tab `index` sits at when nothing is being dragged: the
    // sum of every earlier tab's width plus the spacing between them.
    function xFor(index) {
        let x = 0
        for (let i = 0; i < index; ++i)
            x += root.widthFor(i) + CelestinaTheme.spaceXs
        return x
    }

    function totalWidth() {
        return root.xFor(root.tabs.count)
    }

    // Where a tab centred at `centerX` belongs among every *other* tab, laid
    // out in their own model order. The count of others whose slot starts
    // before that centre is exactly the destination `ListModel.move()` wants:
    // removing the dragged tab and reinserting it there reproduces this order.
    function targetIndexFor(draggedIndex, centerX) {
        let x = 0
        let target = 0
        for (let i = 0; i < root.tabs.count; ++i) {
            if (i === draggedIndex)
                continue
            const width = root.widthFor(i)
            if (centerX >= x + width / 2)
                target += 1
            x += width + CelestinaTheme.spaceXs
        }
        return target
    }

    // Scrolls the viewport so tab `index` is wholly on screen. Selecting a tab
    // that the viewport had scrolled away from must show it, or the window's
    // title changes while the strip appears to point at something else.
    function revealTab(index) {
        const left = root.xFor(index)
        const right = left + root.widthFor(index)
        if (left < viewport.contentX)
            viewport.contentX = left
        else if (right > viewport.contentX + viewport.width)
            viewport.contentX = right - viewport.width
        viewport.clampScroll()
    }

    // Deferred: the delegate for a tab that was just appended may not have
    // measured itself by the time `current` moves onto it.
    onCurrentChanged: Qt.callLater(function() { root.revealTab(root.current) })

    Flickable {
        id: viewport
        anchors.left: parent.left
        anchors.leftMargin: CelestinaTheme.spaceSm
        // Ends where the "+" button starts: exactly the tabs' width while they
        // fit, and everything up to the pinned button once they do not.
        anchors.right: newTabButton.left
        anchors.rightMargin: CelestinaTheme.spaceXs
        anchors.verticalCenter: parent.verticalCenter
        height: CelestinaTheme.controlHeight
        clip: true
        contentWidth: strip.width
        contentHeight: height
        flickableDirection: Flickable.HorizontalFlick
        boundsBehavior: Flickable.StopAtBounds

        // Closing a tab can leave the viewport scrolled past the end of the
        // content that remains; pull it back so the last tab stays flush.
        function clampScroll() {
            viewport.contentX = Math.max(0, Math.min(viewport.contentX,
                                                     viewport.contentWidth - viewport.width))
        }
        onContentWidthChanged: viewport.clampScroll()
        onWidthChanged: viewport.clampScroll()

        // A mouse wheel has no horizontal axis on most devices; its vertical
        // notches scroll the row sideways, the way every tab bar reads them.
        WheelHandler {
            onWheel: function(event) {
                const delta = event.angleDelta.x !== 0 ? event.angleDelta.x
                                                       : event.angleDelta.y
                viewport.contentX -= delta
                viewport.clampScroll()
            }
        }

        Item {
            id: strip
            width: root.totalWidth()
            height: viewport.height

            Repeater {
                id: tabRepeater
                model: root.tabs

                delegate: Rectangle {
                    id: tab
                    required property int index

                    readonly property bool active: index === root.current
                    readonly property bool dragging: dragArea.drag.active
                    // `revision` is read so this re-evaluates when a document
                    // finishes opening and finally has a name.
                    readonly property string label: {
                        root.revision
                        return root.titleFor(index)
                    }
                    readonly property bool dirty: {
                        root.revision
                        return root.dirtyFor(index)
                    }
                    readonly property string displayText: tab.dirty ? tab.label + " •" : tab.label

                    y: 0
                    z: tab.dragging ? 10 : 0
                    x: root.xFor(tab.index)
                    // Own measurement, own width: nothing here reads or writes a
                    // property shared with any other tab, which is what keeps
                    // dragging one from disturbing how the rest lay themselves out.
                    width: labelMetrics.width + root.closeButtonWidth + CelestinaTheme.space2xl
                    height: CelestinaTheme.controlHeight
                    // The tab paints nothing itself: its fill is the suite's one
                    // row recipe below, so an idle tab shows the strip behind it,
                    // a hovered one lights, a held one darkens like any pressed
                    // control and the current one wears the selected surface.
                    color: CelestinaTheme.clear

                    CelestinaRowHighlight {
                        anchors.fill: parent
                        hovered: hover.hovered
                        pressed: dragArea.pressed
                        selected: tab.active
                        selectedFill: CelestinaTheme.surfaceSelected
                    }

                    TextMetrics {
                        id: labelMetrics
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        text: tab.displayText
                    }

                    // Sliding into a slot a drag vacated or filled — never while
                    // being dragged, where the pointer decides `x` directly, and
                    // never with reduced motion, honouring the shared preference.
                    Behavior on x {
                        enabled: !tab.dragging && !CelestinaTheme.reducedMotion
                        NumberAnimation {
                            duration: CelestinaTheme.motionFast
                            easing.type: CelestinaTheme.easeStandard
                        }
                    }

                    Accessible.role: Accessible.PageTab
                    Accessible.name: tab.dirty ? tab.label + ", sin guardar" : tab.label
                    Accessible.selected: tab.active

                    // Non-blocking, so the pointer over the close glyph still
                    // counts as over the tab and the row stays lit beneath it.
                    HoverHandler { id: hover }

                    MouseArea {
                        id: dragArea
                        anchors.fill: parent
                        // Middle click closes, the way every tabbed thing does. A
                        // drag that never crossed the threshold still reaches
                        // onClicked — that disambiguation is what `drag.target`
                        // already does, so no separate click/drag bookkeeping here.
                        acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                        cursorShape: Qt.PointingHandCursor
                        // The viewport is a Flickable, and a Flickable takes
                        // over any horizontal drag it sees unless told not to:
                        // a reorder would turn into a scroll after eight
                        // pixels. The tab keeps its press; empty strip space
                        // still flicks.
                        preventStealing: true
                        drag.target: tab
                        drag.axis: Drag.XAxis
                        drag.minimumX: 0
                        // Bounded by the row's own width, not the window's: a
                        // tab can only be dropped where the row actually ends.
                        drag.maximumX: Math.max(0, strip.width - tab.width)

                        onClicked: function(mouse) {
                            if (mouse.button === Qt.MiddleButton)
                                root.closeRequested(tab.index)
                            else
                                root.selected(tab.index)
                        }

                        onPositionChanged: {
                            if (!drag.active)
                                return
                            const target = root.targetIndexFor(tab.index, tab.x + tab.width / 2)
                            if (target !== tab.index)
                                root.reorderRequested(tab.index, target)
                        }

                        onReleased: {
                            // The drag broke the binding by assigning `x` directly;
                            // put it back so the tab returns to laid-out behaviour.
                            tab.x = Qt.binding(function() { return root.xFor(tab.index) })
                        }
                    }

                    Text {
                        anchors.left: parent.left
                        anchors.leftMargin: CelestinaTheme.spaceSm
                        anchors.verticalCenter: parent.verticalCenter
                        text: tab.displayText
                        elide: Text.ElideMiddle
                        color: tab.active ? CelestinaTheme.text : CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                    }

                    CelestinaIconButton {
                        id: closeButton
                        anchors.right: parent.right
                        anchors.rightMargin: CelestinaTheme.spaceXs
                        anchors.verticalCenter: parent.verticalCenter
                        iconName: "x"
                        // Ghost: at rest the glyph sits bare on the tab. A tonal
                        // fill here painted a rectangle inside every tab and a
                        // second hover surface on top of the tab's own.
                        role: CelestinaButton.Ghost
                        density: CelestinaButton.Compact
                        Accessible.role: Accessible.Button
                        Accessible.name: "Cerrar " + tab.label
                        onClicked: root.closeRequested(tab.index)
                    }
                }
            }
        }
    }

    // Outside the viewport, so it can never scroll away: right after the last
    // tab while the row fits, and pinned to the strip's right edge once the
    // tabs are wider than the space before it.
    CelestinaIconButton {
        id: newTabButton
        readonly property real pinnedX: root.width - CelestinaTheme.spaceSm - width
        x: Math.min(CelestinaTheme.spaceSm + root.totalWidth() + CelestinaTheme.spaceXs,
                    newTabButton.pinnedX)
        anchors.verticalCenter: parent.verticalCenter
        iconName: "plus"
        Accessible.role: Accessible.Button
        Accessible.name: "Pestaña nueva"
        onClicked: root.newRequested()
    }
}
