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
        objectName: "pathPill"

        property bool editing: false

        // The pill is an editor that happens to show crumbs at rest. Ctrl+L
        // selects the whole path, as an address bar does; a press on the pill
        // only turns the editor on and leaves the caret and the drag to the
        // field itself, which is what makes selecting by sweeping work.
        function beginEditing(selectAll) {
            editing = true
            locationField.text = root.controller.currentPath
            locationField.forceActiveFocus()
            if (selectAll === undefined || selectAll)
                locationField.selectAll()
        }

        // A click on the folder you are already in has nowhere to navigate,
        // so it offers the one thing that name can still do: be replaced.
        // The editor opens with exactly that last segment selected; typing
        // overwrites it, Home and End reach the rest.
        function beginEditingOnName() {
            beginEditing(false)
            const path = locationField.text
            const trimmed = path.replace(/\/+$/, "")
            const cut = trimmed.lastIndexOf("/")
            locationField.select(cut + 1, trimmed.length)
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
        //
        // While the editor is open the shield yields the drag: its zero-
        // threshold handler took a sweep from the text input on the first
        // pixel, so a selection never grew past the caret. Yielding alone is
        // not enough — the file rows behind the pill hold passive grabs of
        // their own and would take the sweep at eight pixels and drag a file
        // — so the pill's own handler below claims the sweep first and turns
        // it into the field's selection.
        CelestinaInputShield { yieldsToHost: pathPill.editing }

        // The editor's sweep. A press lands in the field and places the caret;
        // from the first pixel of movement this handler owns the pointer and
        // extends the selection from that caret to the character under it,
        // exactly what the text input would have done had nothing below it
        // been able to steal the drag.
        DragHandler {
            id: pathSweep
            enabled: pathPill.editing
            target: null
            dragThreshold: 0
            acceptedButtons: Qt.LeftButton
            grabPermissions: PointerHandler.CanTakeOverFromAnything
            property int anchor: 0
            onActiveChanged: if (active) pathSweep.anchor = locationField.cursorPosition
            onCentroidChanged: {
                if (!active)
                    return
                const p = locationField.mapFromItem(pathPill, centroid.position.x,
                                                    centroid.position.y)
                locationField.select(pathSweep.anchor, locationField.positionAt(p.x, p.y))
            }
        }

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

        // The field sits under the crumbs and the pill's own MouseArea, live
        // but transparent while the crumbs are shown. A left press on empty
        // pill turns editing on and is then *refused* here, so Qt carries the
        // same press down to the field: the caret lands where the pointer is
        // and a sweep from there selects, exactly as in any editor. Nothing is
        // re-dispatched or simulated; the one press is simply not eaten.
        MouseArea {
            id: pathMouse
            z: 1
            anchors.fill: parent
            visible: !pathPill.editing
            acceptedButtons: root.pathMenu
                             ? Qt.LeftButton | Qt.RightButton
                             : Qt.LeftButton
            cursorShape: Qt.IBeamCursor
            Accessible.name: "Editar ubicación"
            onPressed: function(mouse) {
                if (mouse.button !== Qt.LeftButton)
                    return
                pathPill.beginEditing(false)
                mouse.accepted = false
            }
            onClicked: function(mouse) {
                if (mouse.button === Qt.RightButton && root.pathMenu) {
                    const point = pathMouse.mapToItem(
                                    root.overlayParent, mouse.x, mouse.y)
                    root.pathMenu.popup(root.overlayParent, point)
                }
            }
        }

        Row {
            id: crumbRow
            z: 1
            anchors.right: parent.right
            anchors.rightMargin: 13
            anchors.verticalCenter: parent.verticalCenter
            visible: !pathPill.editing
            // Each crumb carries its own chevron gap, so the row has none:
            // there is no seam between crumbs for a click to fall through.
            spacing: 0

            Repeater {
                id: crumbRepeater
                // Straight off the property: no bare reads to declare a
                // dependency with, and nothing for a compiler to drop.
                model: pathPill.splitCrumbs(root.controller.pathCrumbs)

                delegate: Item {
                    id: crumb

                    required property var modelData
                    required property int index

                    objectName: "crumb-" + index

                    readonly property int gap: 3
                    // The folder you are in: not a link, an editable name.
                    readonly property bool current:
                            crumb.index === crumbRepeater.count - 1
                    // The whole crumb — chevron, gap and label — is one hit
                    // target the height of the pill's usable band, so no click
                    // between two crumbs lands on the pill and opens editing.
                    width: (crumb.index > 0 ? crumbChevron.width + crumb.gap * 2 : 0)
                           + crumbHit.width
                    height: pathPill.height - 2 * CelestinaTheme.spaceXs
                    anchors.verticalCenter: parent.verticalCenter

                    CelestinaIcon {
                        id: crumbChevron
                        visible: crumb.index > 0
                        x: crumb.gap
                        anchors.verticalCenter: parent.verticalCenter
                        width: Math.round(CelestinaTheme.iconSm
                                          * root.hostWindow.interfaceIconScale)
                        height: width
                        name: "chevron-right"
                        fallbackName: "chevron-right"
                        tone: CelestinaIcon.Secondary
                    }

                    CelestinaRowHighlight {
                        id: crumbHit
                        anchors.right: parent.right
                        anchors.top: parent.top
                        anchors.bottom: parent.bottom
                        width: crumbText.implicitWidth + 12
                        hovered: crumbMouse.containsMouse
                        pressed: crumbMouse.pressed

                        Text {
                            id: crumbText
                            anchors.centerIn: parent
                            // Only the folder you are in takes the accent, and
                            // only once the heading is gone.
                            readonly property bool accented:
                                    crumb.current && root.headingRetired > 0.5
                            text: crumbText.accented
                                  ? crumb.modelData.name.toLocaleUpperCase()
                                  : crumb.modelData.name
                            color: crumb.current
                                   ? CelestinaTheme.text
                                   : CelestinaTheme.textMuted
                            font.family: CelestinaTheme.sansFamily
                            font.weight: crumbText.accented
                                         ? CelestinaTheme.weightDemiBold
                                         : CelestinaTheme.weightRegular
                            font.pixelSize: Math.round(CelestinaTheme.fontRowSecondary * root.hostWindow.interfaceTextScale)
                        }
                    }

                    // Ancestors are links and take the hand; the current
                    // folder is text and takes the I-beam, so the cursor says
                    // which of the two a click will do before it is made.
                    MouseArea {
                        id: crumbMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: crumb.current ? Qt.IBeamCursor
                                                   : Qt.PointingHandCursor
                        onClicked: {
                            if (crumb.current)
                                pathPill.beginEditingOnName()
                            else
                                root.controller.openKey(crumb.modelData.key)
                        }
                    }
                }
            }
        }

        CelestinaTextField {
            id: locationField

            objectName: "locationField"
            anchors.fill: parent
            // Present under the crumbs at all times so a refused press can
            // reach it; only its paint follows `editing`. Assistive technology
            // sees it only while it is really the editor; Tab reaching it *is*
            // one way in, and turns the editor on with the whole path selected,
            // the way Ctrl+L does.
            opacity: pathPill.editing ? 1 : 0
            Accessible.ignored: !pathPill.editing
            selectByMouse: true
            // The I-beam over the editor, stated rather than assumed: the
            // field's own cursor depends on Qt's text-input defaults, and a
            // bar that edits must look like one the moment the pointer is on
            // it, in both states.
            HoverHandler {
                enabled: pathPill.editing
                cursorShape: Qt.IBeamCursor
            }
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
                if (!activeFocus && pathPill.editing) {
                    pathPill.editing = false
                } else if (activeFocus && !pathPill.editing) {
                    // Only a deliberate arrival opens the editor. Focus also
                    // lands here uninvited: when the search field collapses
                    // it is disabled, and Qt hands its focus to the next
                    // focusable item in the scope, which is this one. That
                    // arrival turned the crumbs off and left a blank pill
                    // until the next navigation. Anything but the keyboard
                    // is sent back to the list.
                    if (focusReason === Qt.TabFocusReason
                        || focusReason === Qt.BacktabFocusReason
                        || focusReason === Qt.ShortcutFocusReason)
                        pathPill.beginEditing()
                    else
                        root.viewFocusRequested()
                }
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
        // y lo que sobra es contenido que no debe recibir puntero. And, as
        // there, an open search's sweep is the pill's and becomes a selection.
        CelestinaInputShield { yieldsToHost: root.searchExpanded }

        DragHandler {
            id: searchSweep
            enabled: root.searchExpanded
            target: null
            dragThreshold: 0
            acceptedButtons: Qt.LeftButton
            grabPermissions: PointerHandler.CanTakeOverFromAnything
            property int anchor: 0
            onActiveChanged: if (active) searchSweep.anchor = searchField.cursorPosition
            onCentroidChanged: {
                if (!active)
                    return
                const p = searchField.mapFromItem(searchPill, centroid.position.x,
                                                  centroid.position.y)
                searchField.select(searchSweep.anchor, searchField.positionAt(p.x, p.y))
            }
        }

        // The pill is a field, so any of it is a place to start typing: a
        // click on the glass beside the glyph or the text focuses the search
        // rather than being swallowed by the shield. The I-beam says so.
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.IBeamCursor
            onClicked: root.focusSearch()
        }

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

        // Pinned to the right edge, where the collapsed pill's only glyph
        // sits: expanding grows the pill leftwards and the magnifier stays
        // put under the pointer that just clicked it.
        CelestinaIconButton {
            id: searchButton
            objectName: "searchButton"
            x: parent.width - width - 5
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
            // The glyph must not take the field's focus on the press: a Button
            // does by default, the emptied field then collapsed the pill on
            // focus loss, and `focusSearch` reopened it in the same click —
            // the pill closed and opened at once. With no focus of its own the
            // glyph is a plain toggle: it opens the search, and on an open,
            // empty search it closes it again.
            focusPolicy: Qt.NoFocus
            Accessible.name: root.searchExpanded
                             ? (searchField.text.length > 0 ? qsTr("Enfocar búsqueda")
                                                            : qsTr("Cerrar búsqueda"))
                             : qsTr("Buscar")
            onClicked: {
                if (root.searchExpanded && searchField.text.length === 0
                    && !root.controller.searchActive
                    && !root.controller.searchRunning) {
                    root.searchExpanded = false
                    root.viewFocusRequested()
                } else {
                    root.focusSearch()
                }
            }
        }

        CelestinaTextField {
            id: searchField
            objectName: "searchField"
            x: clearSearchButton.x + clearSearchButton.width + 2
            width: Math.max(0, searchButton.x - x - 2)
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
            x: 5
            anchors.verticalCenter: parent.verticalCenter
            width: 32
            height: 32
            visible: opacity > 0.01
            // A ghost at 2 % opacity still took clicks; it only acts once it
            // can be seen.
            enabled: opacity > 0.5
            opacity: root.searchExpanded ? 1 : 0
            role: CelestinaButton.Ghost
            density: CelestinaButton.Compact
            // Same reason as the magnifier: clearing must not first collapse
            // the pill by stealing the field's focus.
            focusPolicy: Qt.NoFocus
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
