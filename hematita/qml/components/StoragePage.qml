pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.hematita 1.0

// Almacenamiento: the mounted locations with how full each is, and the folder
// being browsed inside one of them. The bar carries where the person is and
// the section's filters and actions, which stay disabled until the scan and
// the actions land. Every word is composed here from tokens; names and paths
// are the filesystem's own, shown raw. QML names rows by index and never
// hands a path back.
Item {
    id: page

    required property HematitaAnalysis analysis
    // What a confirmation will blur beneath itself once the actions land.
    required property Item backdrop

    property var locationRows: []
    property var folderRows: []
    // The folder the rows were last woven for, so a refresh of the same
    // folder keeps the person's place and a new folder starts at its top.
    property string folderKey: ""

    readonly property bool browsing: page.analysis.mode === "browsing"

    function bytesText(bytes) {
        if (bytes >= 1099511627776) return (bytes / 1099511627776).toLocaleString(Qt.locale(), "f", 1) + " TiB"
        if (bytes >= 1073741824) return (bytes / 1073741824).toLocaleString(Qt.locale(), "f", 1) + " GiB"
        if (bytes >= 1048576) return (bytes / 1048576).toLocaleString(Qt.locale(), "f", 1) + " MiB"
        if (bytes >= 1024) return (bytes / 1024).toLocaleString(Qt.locale(), "f", 0) + " KiB"
        return qsTr("%1 B").arg(bytes)
    }

    function weave() {
        const published = page.analysis
        const locationCount = Math.min(published.locationNames.length, published.locationPaths.length,
                                       published.locationKinds.length, published.locationUsed.length,
                                       published.locationTotal.length,
                                       published.locationReadable.length)
        const locations = []
        for (let index = 0; index < locationCount; ++index) {
            const used = published.locationUsed[index]
            const total = published.locationTotal[index]
            locations.push({ kind: published.locationKinds[index],
                             name: published.locationNames[index],
                             path: published.locationPaths[index],
                             share: total > 0 ? used / total : 0,
                             usage: total > 0
                                    ? qsTr("%1 de %2").arg(page.bytesText(used))
                                                      .arg(page.bytesText(total))
                                    : "",
                             readable: published.locationReadable[index] === 1 })
        }
        locationList.keepViewport(function() { page.locationRows = locations })

        const folderCount = Math.min(published.browseNames.length, published.browseKinds.length,
                                     published.browseApparent.length)
        const folder = []
        for (let index = 0; index < folderCount; ++index)
            folder.push({ name: published.browseNames[index],
                          kind: published.browseKinds[index],
                          size: page.bytesText(published.browseApparent[index]) })
        const key = published.crumbPaths.join("\n")
        if (key !== page.folderKey || page.folderRows.length === 0) {
            page.folderKey = key
            folderList.reset(function() { page.folderRows = folder })
        } else {
            folderList.keepViewport(function() { page.folderRows = folder })
        }
    }

    // Entering or leaving swaps the list on show; the keyboard follows it, so
    // Enter and Backspace keep working without another Tab.
    function focusBody() {
        if (page.browsing)
            folderList.takeFocus()
        else
            locationList.takeFocus()
    }

    Connections {
        target: page.analysis
        function onRevisionChanged() { if (page.visible) page.weave() }
    }

    onVisibleChanged: if (page.visible) page.weave()

    Component.onCompleted: page.weave()

    ColumnLayout {
        anchors.fill: parent
        spacing: CelestinaTheme.spaceSm

        // ── Bar ────────────────────────────────────────────────────────
        RowLayout {
            Layout.fillWidth: true
            spacing: CelestinaTheme.spaceSm

            Item {
                Layout.fillWidth: true
                Layout.preferredHeight: crumbs.implicitHeight
                clip: true

                PathCrumbs {
                    id: crumbs

                    anchors.verticalCenter: parent.verticalCenter
                    names: page.analysis.crumbNames
                    onChosen: function(index) { page.analysis.enterCrumb(index) }
                }
            }

            // The filters work on a scan's result, which S1-C brings.
            CelestinaCapsule {
                spacing: CelestinaTheme.spaceXs
                inset: CelestinaTheme.spaceXs

                CelestinaIconButton {
                    iconName: "copy"
                    helpText: qsTr("Duplicados")
                    role: CelestinaButton.Ghost
                    checkable: true
                    enabled: false
                }

                CelestinaIconButton {
                    iconName: "folder"
                    helpText: qsTr("Vacías")
                    role: CelestinaButton.Ghost
                    checkable: true
                    enabled: false
                }
            }

            // The actions work on a scan and a selection, which S1-C and S1-D
            // bring.
            CelestinaCapsule {
                spacing: CelestinaTheme.spaceXs
                inset: CelestinaTheme.spaceXs

                CelestinaIconButton {
                    iconName: "gauge"
                    helpText: qsTr("Escanear aquí")
                    role: CelestinaButton.Ghost
                    enabled: false
                }

                CelestinaIconButton {
                    iconName: "folder-open"
                    helpText: qsTr("Abrir en Siderita")
                    role: CelestinaButton.Ghost
                    enabled: false
                }

                CelestinaIconButton {
                    iconName: "user-trash"
                    helpText: qsTr("Papelera")
                    role: CelestinaButton.Ghost
                    enabled: false
                }

                CelestinaIconButton {
                    iconName: "x"
                    helpText: qsTr("Borrar definitivamente")
                    role: CelestinaButton.Ghost
                    enabled: false
                }

                CelestinaIconButton {
                    iconName: "info"
                    helpText: qsTr("Detalles")
                    role: CelestinaButton.Ghost
                    enabled: false
                }
            }

            CelestinaIconButton {
                iconName: "view-refresh"
                helpText: qsTr("Actualizar")
                role: CelestinaButton.Ghost
                onClicked: page.analysis.refresh()
            }
        }

        // ── What the page has to say, if anything ──────────────────────
        Text {
            Layout.fillWidth: true
            visible: page.analysis.startFailed
            text: qsTr("No se pudo iniciar la lectura del disco")
            color: CelestinaTheme.danger
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.WordWrap
        }

        // ── The body, by mode ──────────────────────────────────────────
        StackLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: page.browsing ? 1 : 0

            LocationList {
                id: locationList

                locationRows: page.locationRows
                onEntered: function(index) {
                    page.analysis.enter(index)
                    page.focusBody()
                }
            }

            FolderList {
                id: folderList

                folderRows: page.folderRows
                failed: page.analysis.browseFailed
                loading: page.analysis.busy
                onEntered: function(index) {
                    page.analysis.enter(index)
                    page.focusBody()
                }
                onUpRequested: {
                    page.analysis.up()
                    page.focusBody()
                }
            }
        }
    }
}
