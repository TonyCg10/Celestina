import QtQuick
import org.celestina.fluorita 1.0

// The folders the user mapped, and the way to map another one.
//
// This is the library's navigation: the roots are the axis, and the content
// beside it is always exactly the selection made here. It owns no truth — the
// rows and the selection come from the library, and choosing a folder is a
// request the library answers when the desktop does.
//
// The anatomy is the suite's sidebar, the one Siderita already ships: one inset
// panel, an uppercase eyebrow naming the region, and rows that are a rounded
// highlight rather than a card each. Unmapping lives in the row's context menu
// rather than on a button in every row: it is done once a year, and a
// destructive control sitting permanently one stray click from a selection is
// noise that also happens to be dangerous.
Item {
    id: sidebar

    required property FluoritaLibrary library
    // Where the row menu is popped, and what it blurs beneath itself.
    required property Item overlayParent

    signal sourceSelected(int source)
    signal folderRequested
    signal folderRemoved(int source)

    readonly property int inset: CelestinaTheme.spaceMd
    // An icon-height row, not a content row: `rowHeight` is sized for a list of
    // files with two lines of text, and a folder name is one line.
    readonly property int rowHeight: CelestinaTheme.iconSm + CelestinaTheme.spaceLg

    // The panel plus the margins it is inset by, so the host reserves the right
    // width without knowing how the panel is padded.
    implicitWidth: panel.width + sidebar.inset * 2

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
        var locations = sidebar.library.sourceLocations;
        // Defensive: a short column would mean a publication error, and fewer
        // rows are better than rows with undefined fields.
        var count = Math.min(ids.length, names.length, locations.length);
        var woven = [];
        for (var index = 0; index < count; ++index) {
            woven.push({
                source: parseInt(ids[index], 10),
                name: names[index],
                location: locations[index]
            });
        }
        return woven;
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

    CelestinaSurface {
        id: panel

        x: sidebar.inset
        y: sidebar.inset
        width: 220
        height: sidebar.height - sidebar.inset * 2
        role: CelestinaSurface.Panel

        CelestinaSectionLabel {
            id: eyebrow

            x: CelestinaTheme.spaceMd
            y: CelestinaTheme.spaceMd
            width: parent.width - CelestinaTheme.spaceMd * 2
            text: qsTr("CARPETAS")
        }

        ListView {
            id: list

            x: CelestinaTheme.spaceSm
            y: eyebrow.y + eyebrow.height + CelestinaTheme.spaceSm
            width: parent.width - CelestinaTheme.spaceSm * 2
            height: notice.y - y - CelestinaTheme.spaceSm
            clip: true
            activeFocusOnTab: true
            boundsBehavior: Flickable.StopAtBounds
            model: sidebar.rows
            currentIndex: sidebar.indexOfSelection()

            Accessible.role: Accessible.List
            Accessible.name: qsTr("Carpetas mapeadas")

            // The whole library, above the folders, so one arrow key walks from
            // it into them.
            header: SidebarRow {
                width: ListView.view.width
                height: sidebar.rowHeight
                label: qsTr("Todo")
                iconName: "files"
                current: sidebar.library.selectedSource === -1
                onActivated: sidebar.sourceSelected(-1)
            }

            delegate: SidebarRow {
                id: folderRow

                required property var modelData
                required property int index

                width: ListView.view.width
                height: sidebar.rowHeight
                label: folderRow.modelData.name
                description: folderRow.modelData.location
                iconName: "folder"
                current: sidebar.library.selectedSource === folderRow.modelData.source
                onActivated: sidebar.sourceSelected(folderRow.modelData.source)
                onMenuRequested: function(x, y) {
                    const point = folderRow.mapToItem(sidebar.overlayParent, x, y);
                    rowMenu.targetName = folderRow.modelData.name;
                    rowMenu.targetSource = folderRow.modelData.source;
                    rowMenu.popup(sidebar.overlayParent, point.x, point.y);
                }
            }

            Keys.onReturnPressed: list.activateCurrent()
            Keys.onEnterPressed: list.activateCurrent()
            Keys.onSpacePressed: list.activateCurrent()
            // What the row's menu does, for the keyboard.
            Keys.onDeletePressed: list.removeCurrent()

            function activateCurrent() {
                if (list.currentIndex >= 0 && list.currentIndex < sidebar.rows.length)
                    sidebar.sourceSelected(sidebar.rows[list.currentIndex].source);
            }

            function removeCurrent() {
                if (list.currentIndex >= 0 && list.currentIndex < sidebar.rows.length)
                    sidebar.folderRemoved(sidebar.rows[list.currentIndex].source);
            }
        }

        // Why the last folder request could not be answered. A dismissed dialog
        // leaves this empty and says nothing.
        Text {
            id: notice

            x: CelestinaTheme.spaceMd
            width: parent.width - CelestinaTheme.spaceMd * 2
            anchors.bottom: addRow.top
            anchors.bottomMargin: visible ? CelestinaTheme.spaceSm : 0
            visible: sidebar.library.folderNotice.length > 0
            height: visible ? implicitHeight : 0
            text: sidebar.library.folderNotice
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowSecondary
            wrapMode: Text.WordWrap
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        // Mapping another folder reads as one more row rather than as a slab of
        // button: it is the same gesture as picking one of the folders above it.
        SidebarRow {
            id: addRow

            x: CelestinaTheme.spaceSm
            width: parent.width - CelestinaTheme.spaceSm * 2
            height: sidebar.rowHeight
            anchors.bottom: parent.bottom
            anchors.bottomMargin: CelestinaTheme.spaceSm
            iconName: "folder-plus"
            // Honest state: the desktop's chooser can stay open for as long as
            // the person needs, and a row that still said "Add folder…" would
            // look like it had ignored the click.
            label: sidebar.library.choosingFolder
                ? qsTr("Eligiendo…")
                : qsTr("Añadir carpeta…")
            muted: true
            enabled: !sidebar.library.choosingFolder
            description: qsTr("Elige una carpeta para ver los medios compatibles que contiene")
            onActivated: sidebar.folderRequested()
        }
    }

    GlassContextMenu {
        id: rowMenu

        property string targetName: ""
        property int targetSource: -1

        backdropSource: sidebar.overlayParent

        GlassMenuItem {
            text: qsTr("Dejar de mostrar esta carpeta")
            icon.name: "x"
            icon.source: CelestinaTheme.fallbackIcon("x")
            Accessible.description: qsTr(
                "Quita %1 de la biblioteca. Ni la carpeta ni sus archivos se tocan.")
                .arg(rowMenu.targetName)
            onTriggered: sidebar.folderRemoved(rowMenu.targetSource)
        }
    }
}
