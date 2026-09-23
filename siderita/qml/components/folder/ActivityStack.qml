pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── The one transient column ──────────────────────────────────────
    // Everything that is temporary lives here, anchored to the bottom right of
    // the content frame and growing upward: the rings at the base, the notices
    // above them, the errors above everything.
    //
    // Before this there were three surfaces with three placements, two of them
    // full-width bands across the rows whose `y` chained off each other's
    // visibility. One column removes that chain: each layer is a child in
    // order, and the column's own height is what lifts its top away from the
    // bottom bar.
    //
    // The column is **always** top-anchored. A positioner re-anchored while it
    // is alive lays its children out from the wrong edge — that is exactly what
    // left the shell's toast drawing its cards outside their glass — so the
    // upward growth comes from the placement reading `implicitHeight`, never
    // from a ternary on `anchors`.
Item {
    id: stack

    required property var controller
    required property Item backdrop
    // The widest a notice may be: the content frame minus the bottom capsule
    // and the gaps either side of it.
    required property real maxNoticeWidth

    // Read by the placement and the tests rather than reached into. The error
    // aliases are the loaders, so a caller can ask where they sit as well as
    // what they hold.
    readonly property alias dock: rings
    readonly property alias errorNotice: opErrorLoader
    readonly property alias locationErrorNotice: locationErrorLoader
    readonly property alias noticeRepeater: notices

    implicitWidth: column.implicitWidth
    implicitHeight: column.implicitHeight

    // One published row, `id\ticon\ttone\trunning\ttext`, taken apart. The
    // text is whatever follows the fourth tab: a file name may legally contain
    // one, and the four fields before it never can.
    function cut(index) {
        const empty = { id: "", icon: "", tone: "info", running: false, text: "" }
        const rows = stack.controller.noticeRows
        if (rows === undefined || index < 0 || index >= rows.length)
            return empty
        let rest = rows[index]
        const fields = []
        for (let field = 0; field < 4; field++) {
            const tab = rest.indexOf("\t")
            if (tab < 0)
                return empty
            fields.push(rest.substring(0, tab))
            rest = rest.substring(tab + 1)
        }
        return {
            id: fields[0],
            icon: fields[1],
            tone: fields[2],
            running: fields[3] === "1",
            text: rest
        }
    }

    // Escape dismisses the error holding the top of the column. The rings own
    // their own Escape for the callout; this one only fires when there is an
    // error to clear, so the two never compete.
    Shortcut {
        sequence: "Escape"
        enabled: stack.controller.opError.length > 0
                 || stack.controller.errorText.length > 0
        onActivated: {
            if (stack.controller.opError.length > 0)
                stack.controller.dismissNotice("op-error")
            else
                stack.controller.dismissNotice("error")
        }
    }

    Column {
        id: column
        // Always top-anchored, never a ternary: see the note above.
        anchors.top: parent.top
        anchors.right: parent.right
        spacing: CelestinaTheme.spaceXs

        // Loaders, not plain instances: a notice constructed at startup with
        // empty text would show at once, burn its ten seconds and retire, and
        // the first real error would then arrive at an object that had already
        // retired. Loading it with the error and unloading it with the error
        // also makes the exit come out right — the fade finishes, `dismissed`
        // clears the property, and only then does the loader empty.
        //
        // `visible: active` as well as `active`: an inactive loader is a
        // zero-sized visible child, and the column would still spend a gap on
        // it.
        Loader {
            id: locationErrorLoader
            x: column.width - width
            active: stack.controller.errorText.length > 0
            visible: active
            sourceComponent: ActivityNotice {
                noticeId: "error"
                text: stack.controller.errorText
                iconName: "circle-alert"
                danger: true
                running: false
                maxWidth: stack.maxNoticeWidth
                backdrop: stack.backdrop
                onDismissed: stack.controller.dismissNotice("error")
            }
        }

        Loader {
            id: opErrorLoader
            x: column.width - width
            active: stack.controller.opError.length > 0
            visible: active
            sourceComponent: ActivityNotice {
                noticeId: "op-error"
                text: stack.controller.opError
                iconName: "circle-alert"
                danger: true
                running: false
                maxWidth: stack.maxNoticeWidth
                backdrop: stack.backdrop
                onDismissed: stack.controller.dismissNotice("op-error")
            }
        }

        Repeater {
            id: notices
            model: stack.controller.noticeRows.length

            ActivityNotice {
                id: entry
                required property int index
                readonly property var parts: stack.cut(entry.index)
                // Right-aligned by x like every other child: anchors are not
                // how a positioner's children are placed, and `parent` is still
                // null while the Repeater constructs this delegate.
                x: column.width - entry.width
                noticeId: entry.parts.id
                text: entry.parts.text
                iconName: entry.parts.icon
                danger: entry.parts.tone === "danger"
                running: entry.parts.running
                maxWidth: stack.maxNoticeWidth
                backdrop: stack.backdrop
                onDismissed: id => stack.controller.dismissNotice(id)
            }
        }

        OperationsDock {
            id: rings
            x: column.width - width
            controller: stack.controller
            backdrop: stack.backdrop
            // Always floating: the column sits over the content by definition,
            // and switching the glass off at the end of the list left it flat
            // and opaque.
            floating: true
            // The column is already as wide as the widest notice may be, so the
            // dock collapses when its rings would not fit beside the capsule.
            availableWidth: stack.maxNoticeWidth
        }
    }
}
