pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

// ─── TopBar ─────────────────────────────────────────────────────────────────
// Migas de pan (editable como campo de ruta) + campo de búsqueda, cada uno una
// pastilla de cristal que flota sobre el contenido. Es un hub:
// expone `activeView` / `floating` / `glassTick` que consumen la tira de
// pestañas, las cabeceras y los bordes de scroll, y `searchText` / `beginEditing`
// / `focusSearch` para los atajos de la vista. El controlador, la vista activa,
// la ventana, dónde abrir menús y el menú de ruta llegan por propiedad; devolver
// el foco a la lista sale como señal.
// ──────────────────────────────────────────────────────────────────────────────
Item {
    id: root

    property var controller
    // How far the heading has retired, 0…1. The path bar takes over from it:
    // the current folder is set in caps and weight as the title fades, so the
    // name never disappears from the window — it changes seat.
    property real headingRetired: 0
    // Whether this location is a phone, and whether it answers. Handed down
    // from the heading, which already works both out; asking twice would be a
    // second definition of the same thing.
    // Where the search glyph sits, measured from this bar's right edge, so a
    // control placed below it can line up without reaching inside.
    readonly property real searchCentreFromRight: searchCollapsedWidth / 2
    property bool phoneLocation: false
    property bool phoneConnected: false
    property int phoneIndex: -1
    signal phoneMediaRequested(int index)
    property Item activeView    // fileList o fileGrid según el modo (lo fija quien instancia)
    property var hostWindow
    property Item overlayParent
    property var pathMenu

    // Navigation is a floating contextual layer in the approved material map,
    // so it remains glass even at the top of a folder.
    readonly property bool floating: true

    // Pulsed each time the pills refresh their capture, so the floating tab
    // pills below refresh their glass in the same beat.
    signal glassTick()
    // The search field lives here; the folder view reads / clears it.
    property alias searchText: searchField.text
    property bool searchExpanded: false
    readonly property real searchCollapsedWidth: CelestinaTheme.controlHeightLg
    readonly property real searchExpandedWidth: Math.round(Math.max(
            searchCollapsedWidth,
            Math.min(360, root.width * 0.42, root.width - 188)))
    // Returning focus to the list is the folder view's call (it owns fileList).
    signal viewFocusRequested()

    function beginEditing() { pathPill.beginEditing() }
    function focusSearch() {
        root.searchExpanded = true
        Qt.callLater(function() {
            searchField.forceActiveFocus()
            searchField.selectAll()
        })
    }
    function clearSearch() {
        searchDebounce.stop()
        searchField.text = ""
        root.controller.applyQuery("")
        root.controller.closeSearch()
    }

    function refreshGlass() {
        pathGlass.refreshBackdrop()
        searchGlass.refreshBackdrop()
        root.glassTick()
    }

    onFloatingChanged: if (floating) Qt.callLater(root.refreshGlass)
    onYChanged: if (floating) Qt.callLater(root.refreshGlass)
    onSearchExpandedChanged: if (floating) Qt.callLater(root.refreshGlass)

    // Refresh the blur as content scrolls under the pills; work stops when
    // scrolling stops (no continuous work at rest).
    Connections {
        target: root.activeView
        function onContentYChanged() {
            if (root.floating)
                root.refreshGlass()
        }
    }

    Rectangle {
        id: pathPill

        property bool editing: false

        function beginEditing() {
            editing = true
            locationField.text = root.controller.currentPath
            locationField.forceActiveFocus()
            locationField.selectAll()
        }

        function cancelEditing() {
            editing = false
            root.viewFocusRequested()
        }

        // The crumbs arrive as `key\tname` lines on a published property. QML
        // does not compose paths (ADR 0008): joining components is a path
        // operation, and a crumb has to carry the exact bytes its click
        // navigates to. The key is first because a tab is a legal filename
        // character: it can only ever appear in the name, which is the
        // remainder after the cut.
        function splitCrumbs(lines) {
            const segs = []
            for (let idx = 0; idx < lines.length; idx++) {
                const cut = lines[idx].indexOf("\t")
                if (cut <= 0)
                    continue
                segs.push({
                    key: lines[idx].substring(0, cut),
                    name: lines[idx].substring(cut + 1)
                })
            }
            return segs
        }

        x: 0
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(180, searchPill.x - x - 8)
        height: CelestinaTheme.controlHeightLg
        radius: CelestinaTheme.radiusPill
        clip: true
        color: locationField.visualFocus ? CelestinaTheme.inputFillFocus
                                         : CelestinaTheme.inputFill
        border.width: CelestinaTheme.borderHairline
        border.color: root.floating ? CelestinaTheme.clear
                                    : CelestinaTheme.inputBorder

        // La pastilla flota sobre la lista: su MouseArea sólo cubría izquierdo y
        // derecho, así que el hover, el botón central y el arrastre seguían
        // llegando a la fila de detrás.
        CelestinaInputShield { }

        GlassSurface {
            id: pathGlass
            anchors.fill: parent
            backdropSource: root.activeView
            captureEnabled: root.floating
            cornerRadius: parent.radius
            elevation: 2
            opacity: root.floating ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionNormal
                }
            }
        }

        MouseArea {
            id: pathMouse
            anchors.fill: parent
            visible: !pathPill.editing
            acceptedButtons: root.pathMenu
                             ? Qt.LeftButton | Qt.RightButton
                             : Qt.LeftButton
            cursorShape: Qt.IBeamCursor
            Accessible.name: "Editar ubicación"
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton && root.pathMenu) {
                    const point = pathMouse.mapToItem(
                                    root.overlayParent, mouse.x, mouse.y)
                    root.pathMenu.popup(root.overlayParent, point)
                } else {
                    pathPill.beginEditing()
                }
            }
        }

        Row {
            id: crumbRow
            anchors.right: parent.right
            anchors.rightMargin: 13
            anchors.verticalCenter: parent.verticalCenter
            visible: !pathPill.editing
            spacing: 3

            Repeater {
                id: crumbRepeater
                // Straight off the property: no bare reads to declare a
                // dependency with, and nothing for a compiler to drop.
                model: pathPill.splitCrumbs(root.controller.pathCrumbs)

                delegate: Row {
                    id: crumb

                    required property var modelData
                    required property int index

                    spacing: 3
                    anchors.verticalCenter: parent.verticalCenter

                    CelestinaIcon {
                        visible: crumb.index > 0
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.round(CelestinaTheme.iconSm
                                          * root.hostWindow.interfaceIconScale)
                        height: width
                        name: "chevron-right"
                        fallbackName: "chevron-right"
                        tone: CelestinaIcon.Secondary
                    }

                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: crumbText.implicitWidth + 12
                        height: 24
                        radius: CelestinaTheme.radiusSm
                        color: crumbMouse.containsMouse
                               ? CelestinaTheme.surfaceHover
                               : CelestinaTheme.clear

                        Text {
                            id: crumbText
                            anchors.centerIn: parent
                            readonly property bool current:
                                    crumb.index === crumbRepeater.count - 1
                            // Only the folder you are in takes the accent, and
                            // only once the heading is gone.
                            readonly property bool accented:
                                    crumbText.current && root.headingRetired > 0.5
                            text: crumbText.accented
                                  ? crumb.modelData.name.toLocaleUpperCase()
                                  : crumb.modelData.name
                            color: crumbText.current
                                   ? CelestinaTheme.text
                                   : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.weight: crumbText.accented
                                         ? CelestinaTheme.weightDemiBold
                                         : CelestinaTheme.weightRegular
                            font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary * root.hostWindow.interfaceTextScale)
                            Behavior on font.pixelSize {
                                NumberAnimation { duration: CelestinaTheme.motionFast }
                            }
                        }

                        MouseArea {
                            id: crumbMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.controller.openKey(
                                           crumb.modelData.key)
                        }
                    }
                }
            }
        }

        CelestinaTextField {
            id: locationField

            anchors.fill: parent
            visible: pathPill.editing
            leftPadding: CelestinaTheme.compTextFieldPaddingHorizontal
            rightPadding: CelestinaTheme.compTextFieldPaddingHorizontal
            color: CelestinaTheme.text
            selectionColor: CelestinaTheme.accentPressed
            selectedTextColor: CelestinaTheme.accentInk
            font.family: CelestinaTheme.monoFamily
            font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary * root.hostWindow.interfaceTextScale)
            background: null
            Accessible.name: "Ubicación"

            onActiveFocusChanged: {
                if (!activeFocus && pathPill.editing)
                    pathPill.editing = false
            }

            onAccepted: {
                const location = text
                pathPill.editing = false
                root.controller.openLocation(location)
                root.viewFocusRequested()
            }

            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    pathPill.cancelEditing()
                    event.accepted = true
                }
            }
        }

        Connections {
            target: root.controller

            function onCurrentPathChanged() {
                if (pathPill.editing)
                    pathPill.editing = false
            }
        }
    }

    CelestinaFocusRing {
        target: pathPill
        cornerRadius: pathPill.radius
        shown: locationField.visualFocus
    }

    Rectangle {
        id: searchPill
        width: root.searchExpanded ? root.searchExpandedWidth
                                   : root.searchCollapsedWidth
        height: CelestinaTheme.controlHeightLg
        x: root.width - width
        anchors.verticalCenter: parent.verticalCenter
        radius: CelestinaTheme.radiusPill
        clip: true
        color: searchField.visualFocus
               ? CelestinaTheme.inputFillFocus : CelestinaTheme.inputFill
        border.width: CelestinaTheme.borderHairline
        border.color: root.floating ? CelestinaTheme.clear
                                    : CelestinaTheme.inputBorder

        Behavior on width {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionNormal
                easing.type: CelestinaTheme.easeStandard
            }
        }

        // Igual que la de ruta: el campo y los dos botones no llenan la pastilla,
        // y lo que sobra es contenido que no debe recibir puntero.
        CelestinaInputShield { }

        GlassSurface {
            id: searchGlass
            anchors.fill: parent
            backdropSource: root.activeView
            captureEnabled: root.floating
            cornerRadius: parent.radius
            elevation: 2
            opacity: root.floating ? 1 : 0
            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionNormal
                }
            }
        }

        CelestinaIconButton {
            id: searchButton
            x: 5
            anchors.verticalCenter: parent.verticalCenter
            width: 32
            height: 32
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            // This is a simple UI glyph, so use the bundled monochrome shape.
            // Several native themes ship a coloured search badge; tinting that
            // bitmap turns it into an opaque disc instead of a magnifier.
            iconName: ""
            fallbackIcon: "search"
            Accessible.name: root.searchExpanded ? "Enfocar búsqueda" : "Buscar"
            onClicked: root.focusSearch()
        }

        CelestinaTextField {
            id: searchField
            x: searchButton.x + searchButton.width + 2
            width: Math.max(0, clearSearchButton.x - x - 2)
            height: parent.height
            anchors.verticalCenter: parent.verticalCenter
            visible: opacity > 0.01
            enabled: root.searchExpanded
            opacity: root.searchExpanded ? 1 : 0
            placeholderText: "Buscar aquí y en subcarpetas"
            color: CelestinaTheme.text
            placeholderTextColor: CelestinaTheme.textMuted
            selectionColor: CelestinaTheme.accentPressed
            selectedTextColor: CelestinaTheme.accentInk
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody
                                       * root.hostWindow.interfaceTextScale)
            leftPadding: CelestinaTheme.spaceXs
            rightPadding: CelestinaTheme.spaceXs
            background: null

            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionFast
                }
            }

            // Typing always searches — a recursive walk grouped into "in this
            // folder" and "in subfolders"; clearing it exits search.
            onTextEdited: searchDebounce.restart()
            onTextChanged: {
                if (text.length === 0 && !activeFocus
                    && !root.controller.searchActive
                    && !root.controller.searchRunning)
                    root.searchExpanded = false
            }
            onActiveFocusChanged: {
                if (activeFocus)
                    return
                // Defer until a click on the clear button has had a chance to
                // run; clicking anywhere else collapses an unused search pill.
                Qt.callLater(function() {
                    if (!searchField.activeFocus && searchField.text.length === 0
                        && !root.controller.searchActive
                        && !root.controller.searchRunning)
                        root.searchExpanded = false
                })
            }
            onAccepted: if (text.trim().length > 0)
                            root.controller.searchRecursive(text)

            Keys.onPressed: function(event) {
                if (event.key !== Qt.Key_Escape)
                    return
                if (text.length > 0 || root.controller.searchActive) {
                    root.clearSearch()
                    searchField.forceActiveFocus()
                } else {
                    root.searchExpanded = false
                    root.viewFocusRequested()
                }
                event.accepted = true
            }
        }

        CelestinaIconButton {
            id: clearSearchButton
            x: parent.width - width - 5
            anchors.verticalCenter: parent.verticalCenter
            width: 32
            height: 32
            visible: opacity > 0.01
            opacity: root.searchExpanded ? 1 : 0
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            iconName: ""
            fallbackIcon: "x"
            Accessible.name: searchField.text.length > 0
                             ? "Limpiar búsqueda" : "Cerrar búsqueda"
            onClicked: {
                if (searchField.text.length > 0
                    || root.controller.searchActive) {
                    root.clearSearch()
                    searchField.forceActiveFocus()
                } else {
                    root.searchExpanded = false
                    root.viewFocusRequested()
                }
            }

            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionFast
                }
            }
        }
    }

    CelestinaFocusRing {
        target: searchPill
        cornerRadius: searchPill.radius
        shown: searchField.visualFocus
    }

    Timer {
        id: searchDebounce
        interval: 220
        repeat: false
        onTriggered: {
            if (searchField.text.trim().length > 0)
                root.controller.searchRecursive(searchField.text)
            else
                root.controller.closeSearch()
        }
    }
}
