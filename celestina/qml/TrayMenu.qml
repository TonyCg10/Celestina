// Another application's menu, in this panel's surface.
//
// The entries are that application's, read over `com.canonical.dbusmenu` and
// already bounded by the host; this only draws them and reports which one was
// chosen. What choosing does is the application's business — the panel learns
// nothing back, which is why nothing here waits for a result.
//
// Submenus are drawn indented in place rather than as menus that open sideways.
// The host flattens the tree it was given, and a sideways menu is a second
// surface to place, dismiss and return focus from: that deserves its own
// decision rather than one made in passing.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

SoftMenu {
    id: root

    // The entries the host read, already stripped, bounded and flattened.
    // `var` is necessary: QML has no typed map-list.
    required property var entries
    signal chosen(int entryId)

    title: qsTr("Menú de la bandeja")
    itemSpacing: CelestinaTheme.spaceSm
    headerBodyGap: CelestinaTheme.spaceMd
    rowVerticalInset: CelestinaTheme.spaceXs

    // GlassContextMenu's content item is Qt's real ListView. The controller
    // caps this card to the output while AnchoredMenu retains the complete
    // natural row height, so the ListView owns wheel and keyboard scrolling.
    // This bar only makes that existing viewport visible and draggable.
    readonly property Flickable menuViewport: root.menu.contentItem as Flickable
    // Header, section label and one real application action are the smallest
    // useful view. A request at the output's last pixel may move upward enough
    // to keep these visible rather than creating a one-pixel viewport.
    readonly property int minimumMenuViewportHeight: {
        let measured = root.menu.topPadding + root.menu.bottomPadding;
        const visibleRows = Math.min(root.menu.count, 3);
        for (let index = 0; index < visibleRows; ++index) {
            const item = root.menu.itemAt(index);
            if (item)
                measured += item.implicitHeight;
        }
        return Math.max(1, Math.ceil(measured));
    }
    CelestinaScrollBar {
        objectName: "celestina-tray-menu-scrollbar"
        // Menu treats visual children of its ListView as model entries. Keep
        // the affordance beside that viewport in the popup item instead, so it
        // cannot enter the application's arrow-key order.
        parent: root.menuViewport ? root.menuViewport.parent : null
        x: root.menuViewport
           ? root.menuViewport.x + root.menuViewport.width - width : 0
        y: root.menuViewport ? root.menuViewport.y : 0
        width: CelestinaTheme.spaceSm
        height: root.menuViewport ? root.menuViewport.height : 0
        surface: root.menuViewport
        z: 10
        Accessible.name: qsTr("Desplazamiento vertical")
    }

    readonly property var displayEntries: {
        const built = [
            {"kind": "header", "text": qsTr("Menú de la bandeja"),
             "subtitle": qsTr("%n acción(es)", "", root.entries.length)},
            {"kind": "section", "text": qsTr("Acciones")}
        ];
        for (let index = 0; index < root.entries.length; ++index)
            built.push({"kind": "external", "row": root.entries[index]});
        return built;
    }

    Instantiator {
        model: root.displayEntries
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: SoftMenuRow {
            id: entry

            required property var modelData

            ink: root.ink
            headerTrailingGap: entry.isHeader
                               ? root.headerBodyGap
                               : 0
            verticalInset: root.rowVerticalInset
            trailingGap: entry.isHeader ? 0 : root.itemSpacing

            readonly property bool isHeader: entry.modelData.kind === "header"
            readonly property bool isSection: entry.modelData.kind === "section"
            readonly property bool isExternal: entry.modelData.kind === "external"
            readonly property var external: entry.isExternal
                                            ? entry.modelData.row : null

            text: entry.isExternal
                  ? (entry.external.separator ? "" : entry.external.label)
                  : entry.modelData.text
            header: entry.isHeader
            sectionLabel: entry.isSection
            separator: entry.isExternal && entry.external.separator
            subtitle: entry.isHeader ? entry.modelData.subtitle : ""
            iconName: {
                if (entry.isHeader)
                    return "app-window";
                if (!entry.isExternal || entry.external.separator)
                    return "";
                return entry.external.iconName !== undefined
                       && entry.external.iconName.length > 0
                       ? entry.external.iconName : "app-window";
            }
            fallbackIcon: "app-window"
            actionable: entry.isExternal && entry.external.enabled
            // Nesting arrives flattened, so depth is shown as depth.
            leftPadding: entry.isHeader ? 0
                         : CelestinaTheme.spaceMd
                           + (entry.isExternal ? entry.external.depth : 0)
                             * CelestinaTheme.spaceMd
            // A toggle the application owns: the panel shows its state and
            // never predicts the next one.
            choice: entry.isExternal && entry.external.toggleType.length > 0
            current: entry.isExternal && entry.external.toggleState === 1
            note: entry.choice
                  ? (entry.current ? qsTr("activado") : qsTr("desactivado"))
                  : ""
            dot: entry.current ? root.ink.accent : CelestinaTheme.clear

            onTriggered: {
                if (entry.isExternal)
                    root.chosen(entry.external.id);
            }
        }

    }

}
