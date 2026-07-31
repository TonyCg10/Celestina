import QtQuick
import org.celestina.fluorita 1.0

// Música: las pistas en el orden que ya decidió la proyección del core
// (artista → álbum → pista), agrupadas por artista.
//
// El bucket «Sin artista» es deliberado: una pista sin etiquetas sigue siendo
// música del usuario y desaparecerla sería peor que admitir que no sabemos de
// quién es.
ListView {
    id: list

    required property FluoritaLibrary library
    signal activated(string path)

    // Igual que la galería: se teje cuando la revisión dice que las cuatro
    // columnas de este escaneo ya están publicadas.
    property var rows: []
    model: list.rows

    Connections {
        target: list.library
        function onRevisionChanged() { list.rows = list.weave(); }
    }

    Component.onCompleted: list.rows = list.weave()

    function weave() {
        var paths = list.library.musicPaths;
        var titles = list.library.musicTitles;
        var artists = list.library.musicArtists;
        var albums = list.library.musicAlbums;
        var count = Math.min(paths.length, titles.length, artists.length, albums.length);
        var woven = [];
        for (var index = 0; index < count; ++index) {
            woven.push({
                path: paths[index],
                title: titles[index],
                artist: artists[index],
                album: albums[index]
            });
        }
        return woven;
    }

    clip: true
    focus: true
    // Entra en la cadena de tabulación: sin esto el foco se queda aquí y las
    // acciones de la cabecera no se pueden alcanzar sin ratón.
    activeFocusOnTab: true
    boundsBehavior: Flickable.StopAtBounds
    spacing: CelestinaTheme.spaceXs

    // El orden ya viene agrupado por artista, así que seccionar no reordena
    // nada: sólo pone el rótulo donde cambia.
    section.property: "artist"
    section.criteria: ViewSection.FullString
    section.delegate: CelestinaSectionLabel {
        required property string section

        width: ListView.view.width
        topPadding: CelestinaTheme.spaceMd
        text: section
    }

    Accessible.role: Accessible.List
    Accessible.name: qsTr("Música")

    delegate: Item {
        id: row

        required property var modelData
        required property int index

        width: ListView.view.width
        height: CelestinaTheme.rowHeight

        Accessible.role: Accessible.ListItem
        Accessible.name: row.modelData.title
        Accessible.description: qsTr("%1 · %2").arg(row.modelData.artist).arg(row.modelData.album)
        Accessible.focusable: true
        Accessible.onPressAction: list.activated(row.modelData.path)

        CelestinaSurface {
            anchors.fill: parent
            role: list.currentIndex === row.index
                ? CelestinaSurface.Selected
                : CelestinaSurface.Content

            contentItem: Row {
                spacing: CelestinaTheme.spaceMd

                CelestinaIcon {
                    anchors.verticalCenter: parent.verticalCenter
                    width: CelestinaTheme.iconMd
                    height: CelestinaTheme.iconMd
                    sourceSize: Qt.size(width, height)
                    name: "file-music"
                    fallbackName: "file"
                }

                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 0

                    Text {
                        text: row.modelData.title
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowTitle
                        elide: Text.ElideRight
                    }

                    Text {
                        text: row.modelData.album
                        color: CelestinaTheme.textMuted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontRowSecondary
                        elide: Text.ElideRight
                    }
                }
            }
        }

        TapHandler {
            acceptedButtons: Qt.LeftButton
            onSingleTapped: {
                list.currentIndex = row.index;
                list.forceActiveFocus();
            }
            onDoubleTapped: list.activated(row.modelData.path)
        }
    }

    Keys.onReturnPressed: list.activateCurrent()
    Keys.onEnterPressed: list.activateCurrent()
    Keys.onSpacePressed: list.activateCurrent()

    function activateCurrent() {
        if (list.currentIndex >= 0 && list.currentIndex < list.model.length)
            list.activated(list.model[list.currentIndex].path);
    }
}
