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
    signal activated(string path)

    readonly property bool hasGallery: view.library.imageCount + view.library.videoCount > 0
    readonly property bool hasMusic: view.library.trackCount > 0
    readonly property bool hasContent: view.hasGallery || view.hasMusic

    RowLayout {
        anchors.fill: parent
        spacing: 0

        LibrarySidebar {
            Layout.fillHeight: true
            Layout.preferredWidth: implicitWidth
            library: view.library
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
                        ? qsTr("Generate %1 thumbnails").arg(view.library.artworkPending)
                        : view.library.artworkState === "cancelling"
                            ? qsTr("Cancelling…")
                            : qsTr("Generating %1 of %2 — cancel")
                                .arg(view.library.artworkDone)
                                .arg(view.library.artworkTotal)
                    role: view.library.artworkState === "generating"
                        ? CelestinaButton.Selected
                        : CelestinaButton.Tonal
                    enabled: view.library.artworkState !== "cancelling"
                    Accessible.name: text
                    Accessible.description: qsTr(
                        "Extracts the frame or cover the shared cache is missing. Ctrl+G")
                    onClicked: view.library.artworkState === "generating"
                        ? view.library.cancelArtwork()
                        : view.library.generateArtwork()
                }
            }

            GalleryGrid {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: view.hasGallery
                library: view.library
                onActivated: function(path) { view.activated(path) }
            }

            MusicList {
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
                onActivated: function(path) { view.activated(path) }
            }

            // Honest states: scanning, empty, or a failure with its reason.
            // None of them pretends there is a grid that does not exist.
            Text {
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: !view.hasContent
                text: view.library.state === "scanning"
                    ? qsTr("Scanning your folders…")
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
}
