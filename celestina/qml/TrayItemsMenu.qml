// The shell's StatusNotifierItem inventory as one stable contextual card.
//
// This is deliberately separate from TrayMenu.qml. The inventory chooses one
// foreign application; TrayMenu renders that application's D-Bus menu on its
// independent child carrier. Changing inventory mode therefore never changes
// either menu's ownership or the service/path identity used by an action.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

SoftMenu {
    id: root

    required property var traySource
    required property var providerSource
    readonly property var items: root.traySource
                                 && root.traySource.items !== undefined
                                 ? root.traySource.items : []
    readonly property var settings: ProviderReading.read(root.providerSource,
                                                         "settings")
    readonly property var preferences: root.settings !== undefined
                                       && root.settings.trayItems !== undefined
                                       ? root.settings.trayItems : []
    readonly property var ledger: root.providerSource
                                  ? root.providerSource.requests : null

    // This is presentation state only. Durable visible/pinned/hidden truth is
    // still owned by the provider snapshot and never changed optimistically.
    property string inventoryMode: "visible"
    property GridView inventoryGrid: null
    property Item visibleModeControl: null
    property Item hiddenModeControl: null
    // A provider-confirmed preference can rebuild the tile that owns keyboard
    // focus. Remember only that explicit request; unrelated live tray changes
    // must not steal focus from the mode selector.
    property string pendingFocusRestoreKey: ""
    property string pendingFocusRestoreMode: ""

    readonly property int gridColumns: 4
    readonly property int inventoryIconSize: CelestinaTheme.iconMd
                                             + CelestinaTheme.spaceXs
    readonly property int tileCellHeight: CelestinaTheme.glyphTile
                                               + CelestinaTheme.controlHeightXs
                                               + CelestinaTheme.spaceSm
    // Three rows are always reserved. Switching modes and empty modes therefore
    // retain one card geometry; a larger inventory scrolls inside this viewport.
    readonly property int inventoryCardHeight:
            CelestinaTheme.spaceMd * 2
            + CelestinaTheme.controlHeightSm
            + CelestinaTheme.spaceSm
            + root.tileCellHeight * 3

    readonly property var preferenceModes: {
        const modes = Object.create(null);
        for (let index = 0; index < root.preferences.length; ++index) {
            const row = root.preferences[index];
            if (row.key !== undefined && row.mode !== undefined)
                modes[row.key] = row.mode;
        }
        return modes;
    }
    readonly property var visibleItems: {
        const selected = [];
        for (let index = 0; index < root.items.length; ++index) {
            if (root.modeFor(root.items[index]) !== "hidden")
                selected.push(root.items[index]);
        }
        return selected;
    }
    readonly property var hiddenItems: {
        const selected = [];
        for (let index = 0; index < root.items.length; ++index) {
            if (root.modeFor(root.items[index]) === "hidden")
                selected.push(root.items[index]);
        }
        return selected;
    }
    readonly property var displayedItems: root.inventoryMode === "hidden"
                                          ? root.hiddenItems
                                          : root.visibleItems

    signal activated(string service, string path, int globalX, int globalY)
    signal secondaryActivated(string service, string path,
                              int globalX, int globalY)
    // The complete global rectangle of the invoking tile, not only a point:
    // the child menu's sideways droplet membrane centres its mouth on it.
    signal itemMenuRequested(string service, string path,
                             int globalX, int globalY,
                             int globalWidth, int globalHeight)

    title: qsTr("Bandeja del sistema")
    headerBodyGap: CelestinaTheme.spaceMd
    preserveRequestedTop: true

    function preferenceKey(item) {
        return item && item.preferenceKey !== undefined
               ? item.preferenceKey : "";
    }

    function modeFor(item) {
        const key = root.preferenceKey(item);
        return root.preferenceModeForKey(key);
    }

    function preferenceModeForKey(key) {
        return key.length > 0 && root.preferenceModes[key] !== undefined
               ? root.preferenceModes[key] : "visible";
    }

    function accessibleTitleFor(item) {
        if (item && item.title !== undefined) {
            const candidate = String(item.title).trim();
            if (candidate.length > 0)
                return candidate;
        }
        return qsTr("Aplicación de la bandeja");
    }

    function setMode(item, mode, restoreFocus) {
        const key = root.preferenceKey(item);
        if (key.length === 0 || !root.ledger)
            return;
        if (restoreFocus === true) {
            root.pendingFocusRestoreKey = key;
            root.pendingFocusRestoreMode = mode;
        }
        root.ledger.send("settings", "tray-item-mode",
                         {"key": key, "mode": mode},
                         "tray-item-mode:" + key, "immediate");
    }

    function itemAtModeIndex(index) {
        return index >= 0 && index < root.displayedItems.length
               ? root.displayedItems[index] : null;
    }

    function actionPointFor(tile) {
        return tile ? tile.mapToGlobal(tile.width / 2, tile.height)
                    : Qt.point(0, 0);
    }

    function activateIndex(index, tile) {
        const item = root.itemAtModeIndex(index);
        if (!item)
            return;
        if (root.inventoryMode === "hidden") {
            root.setMode(item, "visible", true);
            return;
        }
        const at = root.actionPointFor(tile);
        root.activated(item.service, item.path, at.x, at.y);
    }

    function secondaryActivateIndex(index, tile) {
        const item = root.itemAtModeIndex(index);
        if (!item || root.inventoryMode === "hidden")
            return;
        const at = root.actionPointFor(tile);
        root.secondaryActivated(item.service, item.path, at.x, at.y);
    }

    function requestItemMenuAtIndex(index, tile) {
        const item = root.itemAtModeIndex(index);
        if (!item || root.inventoryMode === "hidden")
            return;
        if (tile) {
            const at = tile.mapToGlobal(0, 0);
            root.itemMenuRequested(item.service, item.path,
                                   Math.round(at.x), Math.round(at.y),
                                   Math.round(tile.width),
                                   Math.round(tile.height));
            return;
        }
        root.itemMenuRequested(item.service, item.path, 0, 0, 0, 0);
    }

    function resetInventoryView() {
        const view = root.inventoryGrid;
        if (!view)
            return;
        view.currentIndex = root.displayedItems.length > 0 ? 0 : -1;
        view.positionViewAtBeginning();
    }

    onInventoryModeChanged: Qt.callLater(root.resetInventoryView)
    onDisplayedItemsChanged: Qt.callLater(function() {
        const restoreFocus = root.pendingFocusRestoreKey.length > 0
                && root.pendingFocusRestoreMode.length > 0
                && root.preferenceModeForKey(root.pendingFocusRestoreKey)
                   === root.pendingFocusRestoreMode;
        root.resetInventoryView();
        if (!restoreFocus || !root.menu.visible)
            return;
        root.pendingFocusRestoreKey = "";
        root.pendingFocusRestoreMode = "";
        if (root.displayedItems.length > 0 && root.inventoryGrid) {
            root.inventoryGrid.forceActiveFocus();
            return;
        }
        const selector = root.inventoryMode === "hidden"
                         ? root.hiddenModeControl : root.visibleModeControl;
        if (selector)
            selector.forceActiveFocus();
    })

    // The real Menu keeps one header item and one fixed-height inventory item.
    // Item count and preference changes update only the grid model.
    Instantiator {
        model: 1
        onObjectAdded: (index, object) => root.menu.insertItem(0, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: SoftMenuRow {
            ink: root.ink
            text: root.title
            subtitle: qsTr("%n aplicación(es)", "", root.items.length)
            iconName: "system-tray"
            fallbackIcon: "view-grid"
            header: true
            actionable: false
            headerTrailingGap: root.headerBodyGap
        }
    }

    Instantiator {
        model: 1
        onObjectAdded: (index, object) => root.menu.insertItem(1, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: FocusScope {
            id: inventoryCard

            objectName: "celestina-tray-inventory-card"
            width: root.preferredWidth
                   - root.menu.leftPadding - root.menu.rightPadding
            height: root.inventoryCardHeight
            implicitWidth: width
            implicitHeight: height
            Accessible.ignored: true

            // The nested Buttons own focus outside Menu's delegate view. This
            // shortcut is parented into the inventory surface, so it closes
            // this real Menu without intercepting Escape in its foreign child
            // menu window.
            Shortcut {
                sequence: "Escape"
                context: Qt.WindowShortcut
                enabled: root.menu.visible
                onActivated: root.menu.close()
            }

            Component.onCompleted: {
                root.inventoryGrid = inventoryView;
                root.visibleModeControl = visibleMode;
                root.hiddenModeControl = hiddenMode;
                root.resetInventoryView();
            }
            Component.onDestruction: {
                if (root.inventoryGrid === inventoryView)
                    root.inventoryGrid = null;
                if (root.visibleModeControl === visibleMode)
                    root.visibleModeControl = null;
                if (root.hiddenModeControl === hiddenMode)
                    root.hiddenModeControl = null;
            }

            Item {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceMd

                Item {
                    id: modeLine

                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    height: CelestinaTheme.controlHeightSm

                    Text {
                        objectName: "celestina-tray-applications-heading"
                        anchors.left: parent.left
                        anchors.right: modeSelector.left
                        anchors.rightMargin: CelestinaTheme.spaceSm
                        anchors.verticalCenter: parent.verticalCenter
                        text: qsTr("Aplicaciones")
                        textFormat: Text.PlainText
                        color: root.ink.primary
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        font.weight: CelestinaTheme.weightDemiBold
                        elide: Text.ElideRight
                        Accessible.role: Accessible.Heading
                        Accessible.name: text
                    }

                    Row {
                        id: modeSelector

                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: CelestinaTheme.spaceXs

                        BackdropIconButton {
                            id: visibleMode

                            objectName: "celestina-tray-visible-selector"
                            width: CelestinaTheme.controlHeightXs
                            height: CelestinaTheme.controlHeightXs
                            ink: root.ink
                            iconName: "eye"
                            fallbackIcon: "view-grid"
                            iconSize: CelestinaTheme.iconSm
                            helpText: ""
                            role: root.inventoryMode === "visible"
                                  ? CelestinaButton.Selected
                                  : CelestinaButton.Ghost
                            checkable: true
                            checked: root.inventoryMode === "visible"
                            activeFocusOnTab: true
                            Accessible.name: qsTr("Aplicaciones visibles (%n)",
                                                  "",
                                                  root.visibleItems.length)
                            onClicked: root.inventoryMode = "visible"
                            Keys.onDownPressed: function(event) {
                                if (root.visibleItems.length > 0) {
                                    inventoryView.forceActiveFocus();
                                    event.accepted = true;
                                }
                            }
                        }

                        BackdropIconButton {
                            id: hiddenMode

                            objectName: "celestina-tray-hidden-selector"
                            width: CelestinaTheme.controlHeightXs
                            height: CelestinaTheme.controlHeightXs
                            ink: root.ink
                            iconName: "eye-off"
                            fallbackIcon: "view-grid"
                            iconSize: CelestinaTheme.iconSm
                            helpText: ""
                            role: root.inventoryMode === "hidden"
                                  ? CelestinaButton.Selected
                                  : CelestinaButton.Ghost
                            checkable: true
                            checked: root.inventoryMode === "hidden"
                            activeFocusOnTab: true
                            Accessible.name: qsTr("Aplicaciones ocultas (%n)",
                                                  "",
                                                  root.hiddenItems.length)
                            onClicked: root.inventoryMode = "hidden"
                            Keys.onDownPressed: function(event) {
                                if (root.hiddenItems.length > 0) {
                                    inventoryView.forceActiveFocus();
                                    event.accepted = true;
                                }
                            }
                        }
                    }
                }

                Item {
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: modeLine.bottom
                    anchors.topMargin: CelestinaTheme.spaceSm
                    anchors.bottom: parent.bottom

                    Text {
                        anchors.centerIn: parent
                        width: parent.width - CelestinaTheme.spaceXl * 2
                        visible: root.displayedItems.length === 0
                        text: root.inventoryMode === "hidden"
                              ? qsTr("No hay aplicaciones ocultas")
                              : qsTr("No hay aplicaciones visibles")
                        textFormat: Text.PlainText
                        color: root.ink.muted
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        horizontalAlignment: Text.AlignHCenter
                        wrapMode: Text.WordWrap
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }

                    GridView {
                        id: inventoryView

                        objectName: "celestina-tray-icon-grid"
                        anchors.fill: parent
                        // Reserve the scroll affordance in both modes so a
                        // mode with overflow does not shift the icon columns.
                        anchors.rightMargin: CelestinaTheme.spaceSm
                        visible: root.displayedItems.length > 0
                        clip: true
                        model: root.displayedItems
                        cellWidth: Math.floor(width / root.gridColumns)
                        cellHeight: root.tileCellHeight
                        keyNavigationEnabled: true
                        keyNavigationWraps: true
                        boundsBehavior: Flickable.StopAtBounds
                        reuseItems: true
                        activeFocusOnTab: true
                        Accessible.role: Accessible.List
                        Accessible.name: root.inventoryMode === "hidden"
                                         ? qsTr("Aplicaciones ocultas")
                                         : qsTr("Aplicaciones visibles")

                        function activateCurrent() {
                            root.activateIndex(inventoryView.currentIndex,
                                               inventoryView.currentItem);
                        }

                        Keys.onReturnPressed: function(event) {
                            inventoryView.activateCurrent();
                            event.accepted = true;
                        }
                        Keys.onEnterPressed: function(event) {
                            inventoryView.activateCurrent();
                            event.accepted = true;
                        }
                        Keys.onSpacePressed: function(event) {
                            inventoryView.activateCurrent();
                            event.accepted = true;
                        }
                        Keys.onMenuPressed: function(event) {
                            if (inventoryView.currentItem) {
                                root.requestItemMenuAtIndex(
                                    inventoryView.currentIndex,
                                    inventoryView.currentItem
                                );
                                event.accepted = true;
                            }
                        }
                        Keys.onPressed: function(event) {
                            const tile = inventoryView.currentItem;
                            if (!tile)
                                return;
                            if ((event.key === Qt.Key_Return
                                 || event.key === Qt.Key_Enter)
                                && (event.modifiers & Qt.ControlModifier)) {
                                root.secondaryActivateIndex(
                                    inventoryView.currentIndex, tile
                                );
                                event.accepted = true;
                                return;
                            }
                            if (event.key === Qt.Key_F10
                                && (event.modifiers & Qt.ShiftModifier)) {
                                root.requestItemMenuAtIndex(
                                    inventoryView.currentIndex, tile
                                );
                                event.accepted = true;
                            }
                        }

                        delegate: BackdropButton {
                            id: tile

                            required property int index
                            required property var modelData

                            readonly property var trayItem: tile.modelData
                            readonly property bool isHidden:
                                    root.inventoryMode === "hidden"
                            readonly property string accessibleTitle:
                                    root.accessibleTitleFor(tile.trayItem)
                            readonly property string preferenceMode:
                                    root.modeFor(tile.trayItem)
                            readonly property bool canRemember:
                                    root.preferenceKey(tile.trayItem).length > 0
                            readonly property string requestTarget:
                                    tile.canRemember
                                    ? "tray-item-mode:"
                                      + root.preferenceKey(tile.trayItem)
                                    : ""
                            readonly property var outcome: {
                                if (!tile.canRemember || !root.ledger
                                    || root.ledger.revision < 0) {
                                    return {};
                                }
                                return root.ledger.stateOf("settings",
                                                           tile.requestTarget);
                            }
                            readonly property bool waiting:
                                    tile.outcome.state === "pending"
                            readonly property bool failed:
                                    tile.outcome.state === "failed"
                            readonly property url iconSource:
                                    tile.trayItem.iconSource !== undefined
                                    ? tile.trayItem.iconSource : ""
                            readonly property bool externalIconReady:
                                    tile.iconSource.toString().length > 0
                                    && externalIcon.status === Image.Ready
                            readonly property bool externalIconFailed:
                                    tile.iconSource.toString().length > 0
                                    && externalIcon.status === Image.Error

                            objectName: "celestina-tray-icon-tile"
                            width: inventoryView.cellWidth
                                   - CelestinaTheme.spaceXs
                            height: inventoryView.cellHeight
                                    - CelestinaTheme.spaceXs
                            ink: root.ink
                            text: ""
                            helpText: ""
                            // This icon tile owns its content geometry. The
                            // text-button padding would leave only 41 px for
                            // two 30 px actions and overlap pin with eye.
                            leftPadding: 0
                            rightPadding: 0
                            topPadding: 0
                            bottomPadding: 0
                            role: GridView.isCurrentItem
                                  ? CelestinaButton.Selected
                                  : CelestinaButton.Ghost
                            activeFocusOnTab: true
                            Accessible.name: tile.accessibleTitle
                            Accessible.description: {
                                let description = tile.isHidden
                                        ? qsTr("Aplicación oculta; actívala para restaurarla")
                                        : qsTr("Aplicación de la bandeja del sistema");
                                if (tile.trayItem.status === "attention")
                                    description += qsTr(", requiere atención");
                                if (tile.waiting)
                                    description += qsTr(", guardando");
                                if (tile.failed)
                                    description += qsTr(", no se pudo guardar");
                                return description;
                            }

                            onFailedChanged: {
                                if (tile.failed
                                    && root.pendingFocusRestoreKey
                                       === root.preferenceKey(tile.trayItem)) {
                                    root.pendingFocusRestoreKey = "";
                                    root.pendingFocusRestoreMode = "";
                                }
                            }

                            onClicked: {
                                inventoryView.currentIndex = tile.index;
                                root.activateIndex(tile.index, tile);
                            }
                            Keys.onMenuPressed: function(event) {
                                root.requestItemMenuAtIndex(tile.index, tile);
                                event.accepted = !tile.isHidden;
                            }
                            Keys.onPressed: function(event) {
                                if ((event.key === Qt.Key_Return
                                    || event.key === Qt.Key_Enter)
                                    && (event.modifiers & Qt.ControlModifier)) {
                                    root.secondaryActivateIndex(tile.index,
                                                                tile);
                                    event.accepted = !tile.isHidden;
                                    return;
                                }
                                if (event.key === Qt.Key_F10
                                    && (event.modifiers & Qt.ShiftModifier)) {
                                    root.requestItemMenuAtIndex(tile.index,
                                                                tile);
                                    event.accepted = !tile.isHidden;
                                }
                            }
                            Keys.onLeftPressed: function(event) {
                                inventoryView.moveCurrentIndexLeft();
                                inventoryView.forceActiveFocus();
                                event.accepted = true;
                            }
                            Keys.onRightPressed: function(event) {
                                inventoryView.moveCurrentIndexRight();
                                inventoryView.forceActiveFocus();
                                event.accepted = true;
                            }
                            Keys.onUpPressed: function(event) {
                                inventoryView.moveCurrentIndexUp();
                                inventoryView.forceActiveFocus();
                                event.accepted = true;
                            }
                            Keys.onDownPressed: function(event) {
                                inventoryView.moveCurrentIndexDown();
                                inventoryView.forceActiveFocus();
                                event.accepted = true;
                            }

                            contentItem: Item {
                                Item {
                                    id: iconFrame

                                    anchors.horizontalCenter: parent.horizontalCenter
                                    anchors.top: parent.top
                                    anchors.topMargin: CelestinaTheme.spaceXs
                                    width: CelestinaTheme.glyphTile
                                    height: width

                                    Image {
                                        id: externalIcon

                                        objectName: "celestina-tray-tile-external-icon"
                                        anchors.centerIn: parent
                                        width: root.inventoryIconSize
                                        height: width
                                        visible: tile.externalIconReady
                                        source: tile.iconSource
                                        sourceSize.width: root.inventoryIconSize
                                        sourceSize.height: root.inventoryIconSize
                                        fillMode: Image.PreserveAspectFit
                                        asynchronous: true
                                        smooth: true
                                    }

                                    CelestinaIcon {
                                        objectName: "celestina-tray-tile-fallback-icon"
                                        anchors.centerIn: parent
                                        width: root.inventoryIconSize
                                        height: width
                                        visible: !tile.externalIconReady
                                        name: "app-window"
                                        fallbackName: "app-window"
                                        tintOverride: tile.trayItem.status === "attention"
                                                      ? root.ink.accent
                                                      : root.ink.primary
                                        Accessible.ignored: true
                                    }

                                    Rectangle {
                                        anchors.right: parent.right
                                        anchors.bottom: parent.bottom
                                        width: CelestinaTheme.compStatusIndicatorSize
                                        height: width
                                        radius: width / 2
                                        visible: tile.trayItem.status === "attention"
                                        color: root.ink.danger
                                        Accessible.ignored: true
                                    }
                                }

                                Rectangle {
                                    anchors.left: parent.left
                                    anchors.top: parent.top
                                    anchors.margins: CelestinaTheme.spaceXs
                                    width: CelestinaTheme.compStatusIndicatorSize
                                    height: width
                                    radius: width / 2
                                    visible: tile.waiting || tile.failed
                                    color: tile.failed ? root.ink.danger
                                                       : root.ink.warning
                                    Accessible.ignored: true
                                }

                                Item {
                                    anchors.horizontalCenter: parent.horizontalCenter
                                    anchors.bottom: parent.bottom
                                    width: tile.width
                                    height: CelestinaTheme.controlHeightXs

                                    BackdropIconButton {
                                        objectName: "celestina-tray-tile-pin"
                                        x: 0
                                        width: CelestinaTheme.controlHeightXs
                                        height: width
                                        visible: !tile.isHidden
                                        ink: root.ink
                                        iconName: "pin"
                                        helpText: tile.preferenceMode === "pinned"
                                                  ? qsTr("Quitar de la barra")
                                                  : qsTr("Mostrar en la barra")
                                        enabled: tile.canRemember && !tile.waiting
                                        role: tile.preferenceMode === "pinned"
                                              ? CelestinaButton.Selected
                                              : CelestinaButton.Ghost
                                        activeFocusOnTab: true
                                        Accessible.description: tile.accessibleTitle
                                        onClicked: {
                                            inventoryView.currentIndex = tile.index;
                                            root.setMode(
                                                tile.trayItem,
                                                tile.preferenceMode === "pinned"
                                                ? "visible" : "pinned",
                                                true
                                            );
                                        }
                                    }

                                    BackdropIconButton {
                                        objectName: "celestina-tray-tile-visibility"
                                        x: tile.isHidden
                                           ? (parent.width - width) / 2
                                           : parent.width - width
                                        width: CelestinaTheme.controlHeightXs
                                        height: width
                                        ink: root.ink
                                        iconName: tile.isHidden ? "eye" : "eye-off"
                                        helpText: tile.isHidden
                                                  ? qsTr("Mostrar en la bandeja")
                                                  : qsTr("Ocultar de la bandeja")
                                        enabled: tile.canRemember && !tile.waiting
                                        role: CelestinaButton.Ghost
                                        activeFocusOnTab: true
                                        Accessible.description: tile.accessibleTitle
                                        onClicked: {
                                            inventoryView.currentIndex = tile.index;
                                            root.setMode(tile.trayItem,
                                                         tile.isHidden
                                                         ? "visible" : "hidden",
                                                         true);
                                        }
                                    }
                                }
                            }

                            MouseArea {
                                anchors.fill: parent
                                z: 3
                                acceptedButtons: Qt.MiddleButton | Qt.RightButton
                                onClicked: function(mouse) {
                                    inventoryView.currentIndex = tile.index;
                                    if (mouse.button === Qt.MiddleButton)
                                        root.secondaryActivateIndex(tile.index,
                                                                    tile);
                                    else if (mouse.button === Qt.RightButton)
                                        root.requestItemMenuAtIndex(tile.index,
                                                                    tile);
                                }
                            }
                        }

                        CelestinaScrollBar {
                            anchors.top: parent.top
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            surface: inventoryView
                            Accessible.name: qsTr("Desplazamiento vertical")
                        }
                    }
                }
            }
        }
    }

    Connections {
        target: root.menu

        function onOpened() {
            root.inventoryMode = "visible";
            Qt.callLater(function() {
                root.resetInventoryView();
                if (root.visibleModeControl)
                    root.visibleModeControl.forceActiveFocus();
            });
        }

        function onClosed() {
            root.inventoryMode = "visible";
            root.pendingFocusRestoreKey = "";
            root.pendingFocusRestoreMode = "";
        }
    }
}
