// The compact system-tray opener in the panel.
//
// A handful of foreign controls should not expand inside the 40 px bar. Items
// asking for attention and items explicitly pinned by the person stay visible
// here; the complete live inventory opens on the panel's contextual menu
// surface. An individual item's own D-Bus menu is a separate surface and still
// comes from right-clicking that item.
//
// A pinned or attention item whose foreign icon cannot be resolved keeps a
// fixed-size application glyph. Its full title remains the accessible name;
// an icon failure must never turn one compact bar slot into a text column.
//
// The opener deliberately carries no visible count. Its accessible name keeps
// the complete inventory size, while its distinct inventory glyph communicates
// that more items live behind it without spending a text column in the bar.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Row {
    id: root

    objectName: "celestina-tray-drawer"

    // The tray host's items. `var` is necessary: QML has no typed map-list.
    required property var items
    // Flat durable settings rows: {key, mode}, where absence means ordinary
    // visibility. The provider owns bounds and persistence.
    required property var preferences
    required property BackdropInk ink
    signal activated(string service, string path, int globalX, int globalY)
    signal secondaryActivated(string service, string path, int globalX, int globalY)
    // A right-click asks the host for this item's own menu, with the same
    // opener/icon geometry every other panel-attached surface receives.
    signal menuRequested(string service, string path, string appName,
                         rect openerRect, rect attachmentAnchorRect)
    signal drawerRequested(rect openerRect, rect attachmentAnchorRect)

    readonly property var preferenceModes: {
        const modes = Object.create(null);
        for (let index = 0; index < root.preferences.length; ++index) {
            const row = root.preferences[index];
            if (row.key !== undefined && row.mode !== undefined)
                modes[row.key] = row.mode;
        }
        return modes;
    }

    function modeFor(item) {
        const key = item.preferenceKey !== undefined ? item.preferenceKey : "";
        return key.length > 0 && root.preferenceModes[key] !== undefined
               ? root.preferenceModes[key] : "visible";
    }

    readonly property var shown: {
        const pinned = [];
        const urgent = [];
        for (let index = 0; index < items.length; ++index) {
            const item = items[index];
            const mode = root.modeFor(item);
            if (mode === "hidden")
                continue;
            if (mode === "pinned")
                pinned.push(item);
            else if (item.status === "attention")
                urgent.push(item);
        }
        return pinned.concat(urgent);
    }

    function stableKeyFor(item) {
        const preferenceKey = item.preferenceKey !== undefined
                              ? item.preferenceKey : "";
        return preferenceKey.length > 0
               ? preferenceKey : item.service + "\n" + item.path;
    }

    function syncShownModel() {
        const desired = root.shown;
        const desiredKeys = Object.create(null);
        for (let index = 0; index < desired.length; ++index)
            desiredKeys[root.stableKeyFor(desired[index])] = true;

        for (let target = 0; target < desired.length; ++target) {
            const item = desired[target];
            const key = root.stableKeyFor(item);
            let current = -1;
            for (let index = target; index < shownModel.count; ++index) {
                if (shownModel.get(index).stableKey === key) {
                    current = index;
                    break;
                }
            }
            if (current < 0) {
                shownModel.insert(target, {"stableKey": key,
                                           "trayItem": item,
                                           "present": true});
            } else {
                if (current !== target)
                    shownModel.move(current, target, 1);
                shownModel.setProperty(target, "trayItem", item);
                shownModel.setProperty(target, "present", true);
            }
        }

        let departureStarted = false;
        for (let index = 0; index < shownModel.count; ++index) {
            const row = shownModel.get(index);
            if (!desiredKeys[row.stableKey] && row.present) {
                shownModel.setProperty(index, "present", false);
                departureStarted = true;
            }
        }
        if (departureStarted)
            departureSweep.restart();
    }

    function purgeDeparted() {
        for (let index = shownModel.count - 1; index >= 0; --index) {
            if (!shownModel.get(index).present)
                shownModel.remove(index);
        }
    }

    onShownChanged: root.syncShownModel()
    Component.onCompleted: root.syncShownModel()

    ListModel {
        id: shownModel

        dynamicRoles: true
    }

    Timer {
        id: departureSweep

        interval: CelestinaTheme.reducedMotion
                  ? 0 : CelestinaTheme.motionFast
        repeat: false
        onTriggered: root.purgeDeparted()
    }

    spacing: CelestinaTheme.spaceSm
    visible: items.length > 0

    ListView {
        id: shownItems

        objectName: "celestina-tray-shown-items"
        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceSm
        orientation: ListView.Horizontal
        interactive: false
        clip: false
        width: shownModel.count > 0
               ? shownModel.count * CelestinaTheme.iconSm
                 + (shownModel.count - 1) * spacing
               : 0
        height: CelestinaTheme.iconSm
        model: shownModel

        add: Transition {
            NumberAnimation {
                property: "opacity"
                from: 0
                to: 1
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }

        addDisplaced: Transition {
            NumberAnimation {
                property: "x"
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }

        removeDisplaced: Transition {
            NumberAnimation {
                property: "x"
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
                easing.type: CelestinaTheme.easeStandard
            }
        }

        delegate: Item {
                id: entry

                // Named so an offscreen regression can count what the drawer
                // really instantiated. Nothing reads it at runtime.
                objectName: "celestina-tray-item"

                required property var trayItem
                required property bool present
                readonly property var modelData: trayItem
                readonly property bool isPanelAttachmentSource: true
                property bool menuOpen: false
                // The semantic attachment follows the icon slot, not one
                // rendering branch inside it. The foreign Image is hidden
                // while it loads and whenever the catalogue fallback is the
                // visible glyph; leasing that Image made the host clear the
                // anchor at exactly that moment, so the contextual menu lost
                // both its top membrane and its attached fall.
                readonly property Item attachmentAnchor: trayIconAnchor
                readonly property bool hasIconSource: modelData.iconSource !== undefined
                                                      && modelData.iconSource.length > 0
                readonly property bool iconReady: entry.hasIconSource
                                                  && trayIcon.status === Image.Ready

                function globalRect(item) {
                    const topLeft = item.mapToGlobal(0, 0);
                    const bottomRight = item.mapToGlobal(item.width, item.height);
                    return Qt.rect(Math.min(topLeft.x, bottomRight.x),
                                   Math.min(topLeft.y, bottomRight.y),
                                   Math.abs(bottomRight.x - topLeft.x),
                                   Math.abs(bottomRight.y - topLeft.y));
                }

                function openerGlobalRectNow() {
                    return entry.globalRect(entry);
                }

                function attachmentAnchorGlobalRectNow() {
                    return entry.globalRect(entry.attachmentAnchor);
                }

                function anchorPoint() {
                    return entry.mapToGlobal(0, entry.height);
                }

                function activatePrimary() {
                    const at = entry.anchorPoint();
                    root.activated(entry.modelData.service,
                                   entry.modelData.path, at.x, at.y);
                }

                function requestContextMenu() {
                    root.menuRequested(entry.modelData.service,
                                       entry.modelData.path,
                                       String(entry.modelData.title),
                                       entry.openerGlobalRectNow(),
                                       entry.attachmentAnchorGlobalRectNow());
                }

                width: CelestinaTheme.iconSm
                height: CelestinaTheme.iconSm
                opacity: present ? 1 : 0
                activeFocusOnTab: true
                Accessible.role: Accessible.Button
                Accessible.name: modelData.title
                Accessible.description: modelData.status === "attention"
                                        ? qsTr("Requiere atención") : ""
                Accessible.onPressAction: entry.activatePrimary()
                Keys.onReturnPressed: function(event) {
                    entry.activatePrimary();
                    event.accepted = true;
                }
                Keys.onEnterPressed: function(event) {
                    entry.activatePrimary();
                    event.accepted = true;
                }
                Keys.onSpacePressed: function(event) {
                    entry.activatePrimary();
                    event.accepted = true;
                }
                Keys.onMenuPressed: function(event) {
                    entry.requestContextMenu();
                    event.accepted = true;
                }
                Keys.onPressed: function(event) {
                    if (event.key !== Qt.Key_F10
                        || !(event.modifiers & Qt.ShiftModifier)) {
                        return;
                    }
                    entry.requestContextMenu();
                    event.accepted = true;
                }

                Behavior on opacity {
                    NumberAnimation {
                        duration: CelestinaTheme.reducedMotion
                                  ? 0 : CelestinaTheme.motionFast
                        easing.type: entry.present
                                     ? CelestinaTheme.easeStandard
                                     : CelestinaTheme.easeExit
                    }
                }

                Rectangle {
                    objectName: "celestina-tray-item-feedback"
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: entryPointer.pressed ? root.ink.pressedFill
                                                 : (entryPointer.containsMouse
                                                    || entry.menuOpen)
                                                   ? root.ink.controlFill
                                                   : CelestinaTheme.clear

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.reducedMotion
                                      ? 0 : CelestinaTheme.motionFast
                        }
                    }
                }

                Item {
                    id: trayIconAnchor

                    objectName: "celestina-tray-item-attachment-anchor"
                    anchors.fill: parent

                    Image {
                        id: trayIcon

                        objectName: "celestina-tray-item-image"
                        anchors.fill: parent
                        visible: entry.iconReady
                        source: entry.hasIconSource
                                ? entry.modelData.iconSource : ""
                        // The host already resolved this to the size it is
                        // drawn at; the same size avoids a second resample.
                        sourceSize.width: CelestinaTheme.iconSm
                                          * Screen.devicePixelRatio
                        sourceSize.height: CelestinaTheme.iconSm
                                           * Screen.devicePixelRatio
                        fillMode: Image.PreserveAspectFit
                        asynchronous: true
                        smooth: true
                    }

                    CelestinaIcon {
                        objectName: "celestina-tray-item-fallback-icon"

                        anchors.fill: parent
                        visible: !entry.iconReady
                        name: "app-window"
                        fallbackName: "app-window"
                        tintOverride: root.ink.primary
                        Accessible.ignored: true
                    }
                }

                Rectangle {
                    objectName: "celestina-tray-item-focus"
                    anchors.fill: parent
                    anchors.margins: -CelestinaTheme.borderFocus
                    radius: CelestinaTheme.radiusSm + CelestinaTheme.borderFocus
                    color: CelestinaTheme.clear
                    border.width: CelestinaTheme.borderFocus
                    border.color: root.ink.focus
                    visible: entry.activeFocus
                    z: 1000
                    Accessible.ignored: true
                }

                Rectangle {
                    anchors.top: parent.top
                    anchors.right: parent.right
                    width: 5
                    height: 5
                    radius: CelestinaTheme.radiusPill
                    visible: entry.modelData.status === "attention"
                    color: root.ink.danger
                }

                MouseArea {
                    id: entryPointer
                    objectName: "celestina-tray-item-pointer"

                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                    cursorShape: Qt.PointingHandCursor
                    // The context menu rides the press, for the reason
                    // `WorkspacePill` records: with a contextual surface up,
                    // the release of the first click on the bar is cancelled
                    // by the keyboard focus the compositor pulls off it.
                    // Activating an item stays on the release: it opens
                    // nothing of this shell's, so no focus is in flight.
                    onPressed: (mouse) => {
                        if (mouse.button === Qt.RightButton)
                            entry.requestContextMenu();
                    }
                    onClicked: (mouse) => {
                        const at = entry.mapToGlobal(0, entry.height);
                        if (mouse.button === Qt.RightButton)
                            return;
                        if (mouse.button === Qt.MiddleButton) {
                            root.secondaryActivated(entry.modelData.service,
                                                    entry.modelData.path,
                                                    at.x, at.y);
                            return;
                        }
                        entry.activatePrimary();
                    }
                }
        }
    }

    PanelMenuButton {
        id: trayButton

        objectName: "celestina-tray-toggle"

        anchors.verticalCenter: parent.verticalCenter
        implicitWidth: implicitHeight
        ink: root.ink
        attachmentAnchor: trayGlyph
        role: CelestinaButton.Ghost
        helpText: qsTr("Abrir la bandeja (%1)").arg(root.items.length)
        Accessible.name: helpText

        contentItem: Item {
            implicitWidth: CelestinaTheme.iconSm
            implicitHeight: CelestinaTheme.iconSm

            CelestinaIcon {
                id: trayGlyph

                objectName: "celestina-tray-toggle-icon"

                anchors.centerIn: parent
                width: Math.max(1, Math.min(CelestinaTheme.iconSm,
                                            parent.width, parent.height))
                height: width
                name: "system-tray"
                fallbackName: "system-tray"
                tintOverride: root.ink.primary
                Accessible.ignored: true
            }
        }

        onMenuRequested: (openerRect, attachmentAnchorRect) =>
            root.drawerRequested(openerRect, attachmentAnchorRect)
    }
}
