import QtQuick

// Line numbers beside a text surface, wrapping or not.
//
// Two properties of the job shape this component:
//
// - **Only the visible numbers exist.** A host may show a document of tens of
//   megabytes, so one delegate per logical line would be millions of items for
//   the few dozen a viewport can show. The window is found by binary search
//   over the line offsets and walked forward until it leaves the viewport.
// - **The surface owns the geometry.** A wrapped line occupies several visual
//   rows, and only the text widget knows how many. Each number is placed at
//   `positionToRectangle` of its line's first character, so it stays level
//   with the row that line starts on instead of being computed from a line
//   height this component would have to guess.
//
// `positionToRectangle` reads laid-out geometry, which QML cannot bind to;
// `layoutRevision` is the explicit dependency that stands in for it.
//
// The gutter sits *beside* the scrolled content rather than inside it, so
// turning wrapping off and scrolling sideways cannot carry the numbers out of
// the window. That is why it converts the surface's coordinates to its own by
// subtracting `viewportY` instead of simply sharing an origin with the text.
Item {
    id: root

    // The widget being numbered. Read-only to this component: a gutter that
    // could change the text would be a second editor. A Controls `TextArea`
    // is a `TextEdit`, so a read-only preview pane fits here too.
    required property TextEdit surface
    // The part of the surface the user can see, in surface coordinates.
    required property real viewportY
    required property real viewportHeight

    clip: true

    // A layout so wrong that the walk below never leaves the viewport still
    // stops here. No screen shows this many lines, so the cap can only ever
    // bound a bug, never truncate a real gutter.
    readonly property int windowLimit: 512

    // UTF-16 offset of the first character of every logical line. Held rather
    // than recomputed per lookup, because the search below runs on each scroll.
    // A document always has a first line, even when it is empty.
    property var lineStarts: [0]

    property int layoutRevision: 0

    implicitWidth: widest.implicitWidth

    function offsetOf(line) {
        return root.lineStarts[Math.max(0, Math.min(line, root.lineStarts.length - 1))]
    }

    // Where the given logical line starts, vertically, in surface coordinates.
    function topOf(line) {
        return root.surface.positionToRectangle(root.offsetOf(line)).y
    }

    // The last line that begins at or above `y`. Tops increase with the line
    // number, so the window can be found without touching the lines above it.
    function lineAt(y) {
        let low = 0
        let high = root.lineStarts.length - 1
        while (low < high) {
            const middle = Math.floor((low + high + 1) / 2)
            if (root.topOf(middle) <= y)
                low = middle
            else
                high = middle - 1
        }
        return low
    }

    readonly property int firstLine: {
        root.layoutRevision
        return root.lineAt(root.viewportY)
    }

    readonly property int windowCount: {
        root.layoutRevision
        const limit = root.viewportY + root.viewportHeight
        let count = 0
        let line = root.firstLine
        while (line < root.lineStarts.length && count < root.windowLimit
               && root.topOf(line) < limit) {
            count += 1
            line += 1
        }
        // The caret's line is always numbered, even in a viewport too short to
        // fit a whole row.
        return Math.max(count, 1)
    }

    // The line the caret is on, so its number can be picked out the way the
    // surface already picks out its row. Found by the same search: a wrapped
    // line's later rows sit below its own top and above the next line's, so
    // the caret's y resolves to the logical line it is really in.
    //
    // Read-only or unfocused, there is no caret to mark, and marking one would
    // put emphasis on line 1 of a preview nobody is editing.
    readonly property bool caretMeaningful: !root.surface.readOnly
                                            && root.surface.activeFocus
    readonly property int caretLine: {
        root.layoutRevision
        return root.caretMeaningful ? root.lineAt(root.surface.cursorRectangle.y) : -1
    }

    // Rebuilt from the text itself rather than from the widget's `lineCount`,
    // which counts *visual* rows and would number every wrap.
    function reindex() {
        const text = root.surface.text
        const starts = [0]
        let at = text.indexOf("\n")
        while (at >= 0) {
            starts.push(at + 1)
            at = text.indexOf("\n", at + 1)
        }
        // A text ending in a newline opens a final empty line, which the widget
        // shows and which therefore carries a number like any other.
        root.lineStarts = starts
        root.layoutRevision += 1
    }

    Component.onCompleted: root.reindex()

    Connections {
        target: root.surface

        function onTextChanged() { root.reindex() }
        // A different wrap width or a different text size re-lays the document
        // out without changing a character, so the numbers must be re-placed.
        function onWidthChanged() { root.layoutRevision += 1 }
        function onContentHeightChanged() { root.layoutRevision += 1 }
    }

    // The width the gutter reserves: the widest number it will ever show, so
    // the text does not shift sideways when the document passes a power of ten.
    Text {
        id: widest
        visible: false
        text: "0".repeat(String(root.lineStarts.length).length)
        font.family: root.surface.font.family
        font.pixelSize: root.surface.font.pixelSize
    }

    Repeater {
        model: root.windowCount

        delegate: Text {
            id: number
            required property int index

            readonly property int line: root.firstLine + number.index

            y: {
                root.layoutRevision
                return root.topOf(number.line) - root.viewportY
            }
            width: root.width
            horizontalAlignment: Text.AlignRight
            text: number.line + 1
            color: number.line === root.caretLine
                   ? CelestinaTheme.text : CelestinaTheme.textMuted
            font.family: root.surface.font.family
            font.pixelSize: root.surface.font.pixelSize

            // The caret already tells assistive technology where it is; reading
            // a column of numbers alongside the text would only repeat it.
            Accessible.ignored: true
        }
    }
}
