// What a workspace or a whole monitor holds, without going to any of it.
//
// A capsule folded five workspaces behind one shape, which made the strip
// readable and made those five opaque: *five workspaces, one urgent* cannot say
// whether the thing you are looking for is in them. This card answers that.
//
// It asks the compositor to go somewhere — a workspace, or one window on it —
// and does nothing else. A surface that could also move or close windows would
// be a different feature with a different risk, and this one is opened by a
// glance.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

AnchoredCard {
    id: root

    // The workspaces this card shows: one monitor group, or a single workspace.
    required property var workspaces
    // The host connects to these by name. Going to a workspace and going to one
    // window are different requests to the compositor, so they are two signals
    // rather than one the controller has to disambiguate.
    signal activated(string output, int index)
    signal windowActivated(string windowId)

    // One workspace and a whole group are drawn the same way. They were not, and
    // the difference bought nothing: a lone workspace on bare glass was harder
    // to read than the same rows inside the card, and it made one surface behave
    // two ways for no reason a person could name.
    readonly property int boardWidth: 280
    readonly property int boardHeight: 260
    // Three across before wrapping. A group is five workspaces, so two rows of
    // three is the shape that stays readable without becoming a wall.
    readonly property int perRow: Math.min(3, Math.max(1, root.workspaces.length))
    readonly property int rows: Math.ceil(root.workspaces.length / root.perRow)
    readonly property int frame: CelestinaTheme.spaceLg * 2

    // Every place the keyboard can land, in the order it lands on them: each
    // workspace's own row, then that workspace's windows, board after board.
    //
    // One flat list rather than focus chains through the delegates, because the
    // boards are laid out in a grid and a chain would make "down" mean whatever
    // the grid happened to instantiate next. A list is the order a person reads
    // the card in, which is the order they expect an arrow key to follow.
    readonly property var targets: {
        const result = [];
        for (let index = 0; index < root.workspaces.length; ++index) {
            const workspace = root.workspaces[index];
            result.push({"kind": "workspace", "workspace": workspace});
            const map = workspace.map !== undefined ? workspace.map : null;
            if (map === null)
                continue;

            const columns = map.columns !== undefined ? map.columns : [];
            for (let column = 0; column < columns.length; ++column) {
                const windows = columns[column].windows;
                for (let row = 0; row < windows.length; ++row)
                    result.push({"kind": "window", "window": windows[row]});

            }
            const floating = map.floating !== undefined ? map.floating : [];
            for (let row = 0; row < floating.length; ++row)
                result.push({"kind": "window", "window": floating[row]});

        }
        return result;
    }
    // Where the keyboard is. Negative until an arrow key is pressed, so a card
    // opened by pointer does not paint a focus ring nobody asked for.
    property int cursor: -1
    // What that cursor is on, as something a board can compare against without
    // knowing this file's indexing.
    readonly property string currentKey: {
        if (root.cursor < 0 || root.cursor >= root.targets.length)
            return "";

        const target = root.targets[root.cursor];
        return target.kind === "window"
               ? "window:" + target.window.id
               : "workspace:" + target.workspace.index;
    }

    function step(direction) {
        if (root.targets.length === 0)
            return;

        const from = root.cursor < 0 ? -1 : root.cursor;
        root.cursor = (from + direction + root.targets.length) % root.targets.length;
    }

    function activateCursor() {
        if (root.cursor < 0 || root.cursor >= root.targets.length)
            return;

        const target = root.targets[root.cursor];
        if (target.kind === "window") {
            root.windowActivated(target.window.id !== undefined ? target.window.id : "");
            return;
        }
        root.activated(
            target.workspace.output !== undefined ? target.workspace.output : "",
            target.workspace.index !== undefined ? target.workspace.index : 0
        );
    }

    contentWidth: root.perRow * boardWidth
                  + (root.perRow - 1) * CelestinaTheme.spaceMd + root.frame
    contentHeight: root.rows * boardHeight
                   + (root.rows - 1) * CelestinaTheme.spaceMd + root.frame

    // Declared before the card so the card sits above it.
    //
    // Stacking follows declaration order among siblings, and a `z` inside a
    // child orders it against its own siblings rather than against its parent's.
    // Written last, this layer covered the card and swallowed every click meant
    // for a row — which looks exactly like a feature that does not work rather
    // than one that is merely in front of itself.
    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: root.dismissed()
        // Down and up walk the card in reading order; Return and Enter take the
        // place the cursor is on. Space is deliberately not bound: this surface
        // has no control that owns it, and binding it here would make one row
        // answer a key every other row ignores.
        Keys.onDownPressed: root.step(1)
        Keys.onUpPressed: root.step(-1)
        Keys.onReturnPressed: root.activateCursor()
        Keys.onEnterPressed: root.activateCursor()

        MouseArea {
            anchors.fill: parent
            // Escape is the keyboard's way out; a press anywhere off the card is
            // the pointer's, and it works because this window covers the whole
            // output rather than just the card.
            onClicked: root.dismissed()
        }

    }

    GlassCard {
        x: root.cardX + root.shadowMargin
        y: root.cardY + root.shadowMargin
        width: root.contentWidth
        height: root.contentHeight
        backdropSource: root.backdrop
    }

    Item {
        x: root.cardX + root.shadowMargin
        y: root.cardY + root.shadowMargin
        width: root.contentWidth
        height: root.contentHeight
        Accessible.role: Accessible.Dialog
        Accessible.name: qsTr("Espacios de trabajo")

        Grid {
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceLg
            columns: root.perRow
            columnSpacing: CelestinaTheme.spaceMd
            rowSpacing: CelestinaTheme.spaceMd

            Repeater {
                model: root.workspaces

                delegate: WorkspaceMapBoard {
                    required property var modelData

                    width: root.boardWidth
                    height: root.boardHeight
                    workspace: modelData
                    current: modelData.active === true
                    currentKey: root.currentKey
                    onActivated: root.activated(
                        modelData.output !== undefined ? modelData.output : "",
                        modelData.index !== undefined ? modelData.index : 0
                    )
                    onWindowActivated: (windowId) => root.windowActivated(windowId)
                }

            }

        }

    }

}
