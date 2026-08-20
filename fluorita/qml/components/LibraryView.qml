import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// The library: the folders you mapped on the left, what is inside the selected
// one on the right.
//
// The panel is not a pair of tabs. Gallery and Music are the two projections of
// the catalogue, and which of them appears is decided by what the selected
// folder actually holds: a folder of photographs is a grid, a folder of music
// is artists and albums, and a folder with both shows both rather than hiding
// half of its contents behind a control nobody would think to look for.
//
// Nothing here decodes: the scan runs on the engine's worker and thumbnails
// come from the shared cache only if they already exist.
Item {
    id: view

    required property FluoritaLibrary library
    signal activated(string key, string name, rect origin, string poster, string kind)

    readonly property bool hasGallery: view.library.imageCount + view.library.videoCount > 0
    readonly property bool hasMusic: view.library.trackCount > 0
    readonly property bool hasContent: view.hasGallery || view.hasMusic

    // The library's content, and the one thing the floating layers blur. It is
    // a sibling of them, never their parent: `GlassSurface` samples the item it
    // is handed, and handing it an ancestor of the card asks it to capture
    // itself — which yields nothing, so the glass silently falls back to a flat
    // fill. Siderita passes its `contentLayer` for exactly this reason.
    RowLayout {
        id: libraryBody

        anchors.fill: parent
        spacing: 0

        LibrarySidebar {
            Layout.fillHeight: true
            Layout.preferredWidth: implicitWidth
            library: view.library
            overlayParent: view
            onSourceSelected: function(source) { view.library.selectSource(source) }
            onFolderRequested: view.library.addFolder()
            onFolderRemoved: function(source) { view.library.removeFolder(source) }
        }

        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.margins: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceMd

            RowLayout {
                Layout.fillWidth: true
                spacing: CelestinaTheme.spaceMd

                // Finding something by name. The typing is debounced and the
                // matching is the domain's: a grid that folded accents in
                // JavaScript would be a second answer to "does this match".
                CelestinaTextField {
                    id: search

                    Layout.preferredWidth: Math.min(280, view.width / 3)
                    placeholderText: qsTr("Buscar")
                    shape: CelestinaTextField.Search
                    text: view.library.query
                    onTextChanged: typing.restart()
                    Keys.onEscapePressed: {
                        search.text = ""
                        view.library.search("")
                    }

                    // Long enough that a word is typed before the library is
                    // re-projected, short enough that it feels like filtering
                    // rather than submitting.
                    Timer {
                        id: typing

                        interval: 150
                        onTriggered: view.library.search(search.text)
                    }
                }

                Text {
                    Layout.fillWidth: true
                    // The centred state below already says why the panel is
                    // empty; printing the same sentence twice reads as a bug.
                    visible: view.hasContent
                    text: view.library.summary
                    color: view.library.truncated
                        ? CelestinaTheme.warning
                        : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                    elide: Text.ElideRight
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }

                // Generating artwork is the only thing here that starts the
                // engine, so it is a decision by the user and not a side effect
                // of opening the window. It disappears when there is nothing to
                // generate.
                CelestinaButton {
                    activeFocusOnTab: visible
                    visible: view.library.artworkPending > 0
                        || view.library.artworkState !== "idle"
                    text: view.library.artworkState === "idle"
                        ? qsTr("Generar %1 miniaturas").arg(view.library.artworkPending)
                        : view.library.artworkState === "cancelling"
                            ? qsTr("Cancelando…")
                            : qsTr("Generando %1 de %2 — cancelar")
                                .arg(view.library.artworkDone)
                                .arg(view.library.artworkTotal)
                    role: view.library.artworkState === "generating"
                        ? CelestinaButton.Selected
                        : CelestinaButton.Tonal
                    enabled: view.library.artworkState !== "cancelling"
                    Accessible.name: text
                    Accessible.description: qsTr(
                        "Extrae el fotograma o la carátula que falta en la caché compartida. Ctrl+G")
                    onClicked: view.library.artworkState === "generating"
                        ? view.library.cancelArtwork()
                        : view.library.generateArtwork()
                }
            }

            GalleryGrid {
                id: galleryGrid

                onPreviewRequested: function(key, origin) {
                    view.previewRequested(key, origin)
                }
                onPreviewDropped: view.previewDropped()

                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: view.hasGallery
                library: view.library
                onActivated: function(key, name, origin, poster, kind) {
                    view.activated(key, name, origin, poster, kind)
                }
                onMenuRequested: function(key, name, x, y) {
                    view.showMenu(galleryGrid, key, name, x, y);
                }
            }

            MusicList {
                id: musicList

                Layout.fillWidth: true
                // A folder holding both keeps the grid above and the tracks
                // below at a readable height, instead of one of them collapsing
                // to nothing.
                Layout.fillHeight: !view.hasGallery
                Layout.preferredHeight: view.hasGallery
                    ? Math.round(view.height / 3)
                    : -1
                visible: view.hasMusic
                library: view.library
                onActivated: function(key, name, origin, poster, kind) {
                    view.activated(key, name, origin, poster, kind)
                }
                onMenuRequested: function(key, name, x, y) {
                    view.showMenu(musicList, key, name, x, y);
                }
            }

            // Honest states: scanning, empty, or a failure with its reason.
            // None of them pretends there is a grid that does not exist.
            Text {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: !view.hasContent
                text: view.library.state === "scanning"
                    ? qsTr("Explorando tus carpetas…")
                    : view.library.summary
                color: view.library.state === "error"
                    ? CelestinaTheme.danger
                    : CelestinaTheme.textFaint
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                wrapMode: Text.WordWrap
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
        }
    }

    // What happened to the last item action. A row disappearing with no word
    // for it reads as a crash, and a Trash move that failed must never look
    // like one that worked.
    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceLg
        width: Math.min(parent.width - CelestinaTheme.spaceLg * 2, 520)
        visible: view.library.itemNotice.length > 0
        text: view.library.itemNotice
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontRowSecondary
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }

    // The shared menu is a real `Menu`: it is popped at a point in a parent's
    // coordinates and the overlay keeps it on screen by itself, so nothing here
    // clamps or positions it.
    // Where the keyboard puts the caret when a person asks to search.
    function focusSearch() {
        search.forceActiveFocus(Qt.ShortcutFocusReason)
        search.selectAll()
    }

    function showMenu(source, key, name, x, y) {
        const point = source.mapToItem(view, x, y);
        itemMenu.targetName = name;
        itemMenu.targetKey = key;
        itemMenu.editable = view.editor.admits(key);
        itemMenu.describable = view.metadata.admits(key);
        itemMenu.popup(view, point.x, point.y);
    }

    // Editing is the host's business, not the library's: it opens a surface
    // over the whole window, so the menu says what was asked for and the window
    // decides what that means. What the view does need is the two objects that
    // know what an item admits, because a menu is built before it is shown.
    required property FluoritaEditor editor
    required property FluoritaMetadata metadata
    signal previewRequested(string key, rect origin)
    signal previewDropped()
    required property FluoritaBatch batch

    signal editRequested(string key)
    signal metadataRequested(string key)

    ItemMenu {
        id: itemMenu

        backdropSource: libraryBody
        onTrashRequested: function(key) { view.library.trashItem(key) }
        onPropertiesRequested: function(key) { view.library.describeItem(key) }
        onEditRequested: function(key) { view.editRequested(key) }
        onMetadataRequested: function(key) { view.metadataRequested(key) }
    }

    // What can be done to the files the person picked out. It sits over the
    // grid because that is where the selection is, and it is only there while
    // there is a selection.
    BatchBar {
        anchors.horizontalCenter: parent.horizontalCenter
        anchors.bottom: parent.bottom
        anchors.bottomMargin: CelestinaTheme.spaceLg
        visible: galleryGrid.selectedKeys.length > 0
        batch: view.batch
        keys: galleryGrid.selectedKeys
        onDismissed: galleryGrid.clearSelection()
    }

    // What a finished run amounted to. The selection is dropped with it: the
    // files it named are not the files that are there now.
    Connections {
        target: view.batch

        function onRunningChanged() {
            if (!view.batch.running && view.batch.notice.length > 0) {
                galleryGrid.clearSelection()
            }
        }
    }

    ItemDetailPanel {
        library: view.library
        backdrop: libraryBody
    }
}
