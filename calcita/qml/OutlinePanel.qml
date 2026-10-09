pragma ComponentBehavior: Bound
import QtQuick
import QtQuick.Pdf
import org.celestina.calcita 1.0

// The outline side card: the document's bookmarks as an indented tree, every
// branch open. A click, or Enter on the current row (Up/Down move it), asks
// to go to the bookmark's place; Escape asks to close. QtPdf reads the
// outline; the window moves the pages.
Item {
    id: panel

    required property PdfDocument document
    // The current row, which Up/Down move and Enter follows.
    property int currentRow: 0
    readonly property int rowCount: tree.rows
    readonly property int rowInset: CelestinaTheme.spaceLg

    // `page` is 0-based; `location` is in points from the page's top left.
    signal bookmarkChosen(int page, point location)
    signal closeRequested()

    // QPdfBookmarkModel's roles, from Qt::UserRole on.
    readonly property int pageRole: Qt.UserRole + 2
    readonly property int locationRole: Qt.UserRole + 3

    function focusTree() {
        tree.forceActiveFocus()
    }

    function choose(row) {
        if (row < 0 || row >= tree.rows)
            return
        panel.currentRow = row
        const index = tree.index(row, 0)
        panel.bookmarkChosen(bookmarks.data(index, panel.pageRole),
                             bookmarks.data(index, panel.locationRole))
    }

    function title(row) {
        return bookmarks.data(tree.index(row, 0), Qt.UserRole)
    }

    objectName: "outlinePanel"

    PdfBookmarkModel {
        id: bookmarks
        document: panel.document
        onModelReset: {
            panel.currentRow = 0
            tree.expandRecursively()
        }
    }

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Grouped
        Accessible.ignored: true
    }

    CelestinaSectionLabel {
        id: heading
        x: panel.rowInset
        y: panel.rowInset
        text: qsTr("Índice")
    }

    Text {
        objectName: "outlineEmpty"
        visible: tree.rows === 0
        x: panel.rowInset
        anchors.top: heading.bottom
        anchors.topMargin: CelestinaTheme.spaceSm
        width: panel.width - panel.rowInset * 2
        wrapMode: Text.WordWrap
        text: qsTr("Este documento no tiene índice.")
        color: CelestinaTheme.textMuted
        font.pixelSize: CelestinaTheme.fontBody
    }

    TreeView {
        id: tree
        objectName: "outlineTree"
        anchors.top: heading.bottom
        anchors.topMargin: CelestinaTheme.spaceSm
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceSm
        clip: true
        model: bookmarks
        boundsBehavior: Flickable.StopAtBounds
        keyNavigationEnabled: false
        pointerNavigationEnabled: false
        activeFocusOnTab: true
        Accessible.role: Accessible.Tree
        Accessible.name: qsTr("Índice")
        Component.onCompleted: tree.expandRecursively()

        Keys.onUpPressed: panel.currentRow = Math.max(0, panel.currentRow - 1)
        Keys.onDownPressed: panel.currentRow = Math.min(tree.rows - 1, panel.currentRow + 1)
        Keys.onReturnPressed: panel.choose(panel.currentRow)
        Keys.onEnterPressed: panel.choose(panel.currentRow)
        Keys.onEscapePressed: panel.closeRequested()

        CelestinaWheelScroll {
            view: tree
        }

        delegate: Item {
            id: outlineRow

            required property int row
            required property int depth
            required property string title

            objectName: "outlineRow" + outlineRow.row
            implicitWidth: tree.width
            implicitHeight: CelestinaTheme.controlHeight
            Accessible.role: Accessible.TreeItem
            Accessible.name: outlineRow.title
            Accessible.onPressAction: panel.choose(outlineRow.row)

            CelestinaRowHighlight {
                anchors.fill: parent
                anchors.leftMargin: CelestinaTheme.spaceXs
                anchors.rightMargin: CelestinaTheme.spaceXs
                hovered: rowMouse.containsMouse
                pressed: rowMouse.pressed
                focused: tree.activeFocus && panel.currentRow === outlineRow.row
            }

            Text {
                x: panel.rowInset + outlineRow.depth * panel.rowInset
                width: outlineRow.width - x - panel.rowInset
                anchors.verticalCenter: parent.verticalCenter
                elide: Text.ElideRight
                text: outlineRow.title
                color: CelestinaTheme.text
                font.pixelSize: CelestinaTheme.fontBody
            }

            MouseArea {
                id: rowMouse
                anchors.fill: parent
                hoverEnabled: true
                onClicked: panel.choose(outlineRow.row)
            }
        }
    }
}
