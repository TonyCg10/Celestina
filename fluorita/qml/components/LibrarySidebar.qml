import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// The folders the user mapped, and the button that maps another one.
//
// This is the library's navigation: the roots are the axis, and the content
// panel beside it always shows exactly the selection made here. It owns no
// truth — the rows and the selection come from the library, and choosing a
// folder is a request the library answers when the desktop does.
Item {
    id: sidebar

    required property FluoritaLibrary library

    // Emitted with a root's handle, or -1 for every root at once.
    signal sourceSelected(int source)
    signal folderRequested
    signal folderRemoved(int source)

    implicitWidth: CelestinaTheme.spaceLg * 13

    // Rebuilt once per publication rather than bound to the three lists: they
    // are published one at a time, and a binding on them would re-run halfway
    // through with the names of one scan beside the paths of another.
    property var rows: []

    Connections {
        target: sidebar.library
        function onRevisionChanged() { sidebar.rows = sidebar.weave(); }
    }

    Component.onCompleted: sidebar.rows = sidebar.weave()

    function weave() {
        var ids = sidebar.library.sourceIds;
        var names = sidebar.library.sourceNames;
        var paths = sidebar.library.sourcePaths;
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        var count = Math.min(ids.length, names.length, paths.length);
        var woven = [];
        for (var index = 0; index < count; ++index) {
            woven.push({
                source: parseInt(ids[index], 10),
                name: names[index],
                path: paths[index]
            });
        }
        return woven;
    }

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Panel

        contentItem: ColumnLayout {
            spacing: CelestinaTheme.spaceSm

            CelestinaSectionLabel {
                Layout.fillWidth: true
                Layout.leftMargin: CelestinaTheme.spaceMd
                Layout.topMargin: CelestinaTheme.spaceMd
                text: qsTr("Folders")
            }

            ListView {
                id: list

                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                activeFocusOnTab: true
                boundsBehavior: Flickable.StopAtBounds
                spacing: CelestinaTheme.spaceXs
                // The whole-library row sits above the folders as index 0, so
                // one arrow key walks from it into them.
                model: sidebar.rows
                currentIndex: sidebar.indexOfSelection()

                Accessible.role: Accessible.List
                Accessible.name: qsTr("Mapped folders")

                header: Item {
                    width: ListView.view.width
                    height: CelestinaTheme.rowHeight

                    CelestinaSurface {
                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceXs
                        role: sidebar.library.selectedSource === -1
                            ? CelestinaSurface.Selected
                            : CelestinaSurface.Content

                        contentItem: Row {
                            leftPadding: CelestinaTheme.spaceMd
                            spacing: CelestinaTheme.spaceMd

                            CelestinaIcon {
                                anchors.verticalCenter: parent.verticalCenter
                                width: CelestinaTheme.iconSm
                                height: width
                                sourceSize: Qt.size(width, height)
                                name: "folder-multiple"
                                fallbackName: "folder"
                            }

                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                text: qsTr("Everything")
                                color: sidebar.library.selectedSource === -1
                                    ? CelestinaTheme.accent
                                    : CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontBody
                                elide: Text.ElideRight
                            }
                        }
                    }

                    Accessible.role: Accessible.ListItem
                    Accessible.name: qsTr("Everything")
                    Accessible.focusable: true
                    Accessible.onPressAction: sidebar.sourceSelected(-1)

                    // A MouseArea stacked above the surface, not a handler on
                    // the item underneath it. `CelestinaSurface` is a `Pane`,
                    // and a Control filling the row takes the press before a
                    // parent's TapHandler ever sees it — which is why clicking
                    // a folder did nothing at all while the keyboard worked.
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: sidebar.sourceSelected(-1)
                    }
                }

                delegate: Item {
                    id: row

                    required property var modelData
                    required property int index

                    width: ListView.view.width
                    height: CelestinaTheme.rowHeight

                    readonly property bool current:
                        sidebar.library.selectedSource === row.modelData.source

                    Accessible.role: Accessible.ListItem
                    Accessible.name: row.modelData.name
                    Accessible.description: qsTr("%1 — Delete stops showing it")
                        .arg(row.modelData.path)
                    Accessible.focusable: true
                    Accessible.onPressAction: sidebar.sourceSelected(row.modelData.source)

                    CelestinaSurface {
                        anchors.fill: parent
                        anchors.margins: CelestinaTheme.spaceXs
                        role: row.current
                            ? CelestinaSurface.Selected
                            : CelestinaSurface.Content

                        contentItem: RowLayout {
                            spacing: CelestinaTheme.spaceMd

                            CelestinaIcon {
                                Layout.leftMargin: CelestinaTheme.spaceMd
                                Layout.alignment: Qt.AlignVCenter
                                width: CelestinaTheme.iconSm
                                height: width
                                sourceSize: Qt.size(width, height)
                                name: "folder"
                                fallbackName: "folder"
                            }

                            Text {
                                Layout.fillWidth: true
                                Layout.alignment: Qt.AlignVCenter
                                text: row.modelData.name
                                color: row.current
                                    ? CelestinaTheme.accent
                                    : CelestinaTheme.text
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontBody
                                font.weight: row.current
                                    ? CelestinaTheme.weightMedium
                                    : CelestinaTheme.weightRegular
                                elide: Text.ElideRight
                            }

                            // Reserves the space the unmap button occupies, so
                            // a long folder name elides before it runs under a
                            // control that sits outside this layout.
                            Item {
                                Layout.rightMargin: CelestinaTheme.spaceSm
                                Layout.preferredWidth: CelestinaTheme.controlHeightXs
                                Layout.preferredHeight: 1
                            }

                        }
                    }

                    // Selecting the row. A MouseArea stacked above the surface,
                    // not a handler on the item underneath it:
                    // `CelestinaSurface` is a `Pane`, and a Control filling the
                    // row takes the press before a parent's TapHandler ever sees
                    // it — which is why clicking a folder did nothing at all
                    // while the keyboard worked.
                    MouseArea {
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: sidebar.sourceSelected(row.modelData.source)
                    }

                    // Unmapping is per row, and it removes the folder from the
                    // library only — never a file from disk.
                    //
                    // Declared last so it stacks above the row's MouseArea and
                    // keeps its own press. Deliberately outside the tab chain:
                    // one button per folder in it meant Tab walked every one of
                    // them before reaching the content, and the key that
                    // activates a focused button is the same Return that opens a
                    // folder — two presses from unmapping one by accident. The
                    // keyboard removes through Delete on the focused row
                    // instead, a gesture nobody arrives at by tabbing.
                    CelestinaIconButton {
                        anchors.right: parent.right
                        anchors.rightMargin: CelestinaTheme.spaceMd
                        anchors.verticalCenter: parent.verticalCenter
                        activeFocusOnTab: false
                        iconName: "x"
                        fallbackIcon: "eraser"
                        role: CelestinaButton.Ghost
                        Accessible.name: qsTr("Stop showing %1").arg(row.modelData.name)
                        Accessible.description: qsTr(
                            "Removes the folder from the library. The folder and its files are not touched.")
                        onClicked: sidebar.folderRemoved(row.modelData.source)
                    }
                }

                Keys.onReturnPressed: list.activateCurrent()
                Keys.onEnterPressed: list.activateCurrent()
                Keys.onSpacePressed: list.activateCurrent()
                // What the row's button does, for the keyboard.
                Keys.onDeletePressed: list.removeCurrent()

                function removeCurrent() {
                    if (list.currentIndex >= 0 && list.currentIndex < sidebar.rows.length)
                        sidebar.folderRemoved(sidebar.rows[list.currentIndex].source);
                }

                function activateCurrent() {
                    if (list.currentIndex >= 0 && list.currentIndex < sidebar.rows.length)
                        sidebar.sourceSelected(sidebar.rows[list.currentIndex].source);
                }
            }

            CelestinaButton {
                Layout.fillWidth: true
                Layout.leftMargin: CelestinaTheme.spaceSm
                Layout.rightMargin: CelestinaTheme.spaceSm
                Layout.bottomMargin: CelestinaTheme.spaceSm
                activeFocusOnTab: true
                role: CelestinaButton.Tonal
                // The label is the honest state: the desktop's dialog can stay
                // open for as long as the person needs, and a button that still
                // said "Add folder…" would look like it had ignored the click.
                text: sidebar.library.choosingFolder
                    ? qsTr("Choosing…")
                    : qsTr("Add folder…")
                enabled: !sidebar.library.choosingFolder
                Accessible.name: text
                Accessible.description: qsTr("Pick a folder to show its supported media")
                onClicked: sidebar.folderRequested()
            }

            // Why the last request could not be answered. A dismissed dialog
            // leaves this empty and says nothing.
            Text {
                Layout.fillWidth: true
                Layout.leftMargin: CelestinaTheme.spaceMd
                Layout.rightMargin: CelestinaTheme.spaceMd
                Layout.bottomMargin: CelestinaTheme.spaceMd
                visible: sidebar.library.folderNotice.length > 0
                text: sidebar.library.folderNotice
                color: CelestinaTheme.danger
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                wrapMode: Text.WordWrap
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }
    }

    // Where the selected root sits among the rows, so the keyboard starts from
    // what is on screen. -1 is the header row, which the view keeps separate.
    function indexOfSelection() {
        for (var index = 0; index < sidebar.rows.length; ++index) {
            if (sidebar.rows[index].source === sidebar.library.selectedSource)
                return index;
        }
        return -1;
    }
}
