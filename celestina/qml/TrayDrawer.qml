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
    // A right-click asks the host for this item's own menu.
    signal menuRequested(string service, string path, int globalX, int globalY)
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

    spacing: CelestinaTheme.spaceSm
    visible: items.length > 0

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

    Row {
        id: shownItems

        anchors.verticalCenter: parent.verticalCenter
        spacing: CelestinaTheme.spaceSm

        Repeater {
            model: root.shown

            delegate: Item {
                id: entry

                // Named so an offscreen regression can count what the drawer
                // really instantiated. Nothing reads it at runtime.
                objectName: "celestina-tray-item"

                required property var modelData
                readonly property bool hasIconSource: modelData.iconSource !== undefined
                                                      && modelData.iconSource.length > 0
                readonly property bool iconReady: entry.hasIconSource
                                                  && trayIcon.status === Image.Ready

                function anchorPoint() {
                    return entry.mapToGlobal(0, entry.height);
                }

                function activatePrimary() {
                    const at = entry.anchorPoint();
                    root.activated(entry.modelData.service,
                                   entry.modelData.path, at.x, at.y);
                }

                function requestContextMenu() {
                    const at = entry.anchorPoint();
                    root.menuRequested(entry.modelData.service,
                                       entry.modelData.path, at.x, at.y);
                }

                width: CelestinaTheme.iconSm
                height: CelestinaTheme.iconSm
                anchors.verticalCenter: parent.verticalCenter
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

                Rectangle {
                    objectName: "celestina-tray-item-feedback"
                    anchors.fill: parent
                    radius: CelestinaTheme.radiusSm
                    color: entryPointer.pressed ? root.ink.pressedFill
                                                 : entryPointer.containsMouse
                                                   ? root.ink.controlFill
                                                   : CelestinaTheme.clear

                    Behavior on color {
                        ColorAnimation {
                            duration: CelestinaTheme.reducedMotion
                                      ? 0 : CelestinaTheme.motionFast
                        }
                    }
                }

                Image {
                    id: trayIcon

                    objectName: "celestina-tray-item-image"
                    anchors.fill: parent
                    visible: entry.iconReady
                    source: entry.hasIconSource ? entry.modelData.iconSource : ""
                    // The host already resolved this to the size it is drawn
                    // at; the same size avoids a second resample.
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
                    onClicked: (mouse) => {
                        const at = entry.mapToGlobal(0, entry.height);
                        if (mouse.button === Qt.RightButton) {
                            entry.requestContextMenu();
                            return;
                        }
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
    }
}
