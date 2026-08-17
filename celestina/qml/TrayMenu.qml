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
    required property string appName
    signal chosen(int entryId)

    title: qsTr("Menú de %1").arg(root.appName)
    itemSpacing: CelestinaTheme.spaceSm
    headerBodyGap: CelestinaTheme.spaceMd
    rowVerticalInset: CelestinaTheme.spaceXs

    // GlassContextMenu's content item is Qt's real ListView. The controller
    // caps this card to the output while AnchoredMenu retains the complete
    // natural row height, so the ListView owns wheel and keyboard scrolling.
    readonly property Flickable menuViewport: root.menu.contentItem as Flickable
    // Header, section label and one real application action are the smallest
    // useful view. A request at the output's last pixel may move upward enough
    // to keep these visible rather than creating a one-pixel viewport.
    readonly property int minimumMenuViewportHeight: {
        let measured = root.menu.topPadding + root.menu.bottomPadding;
        const visibleRows = Math.min(root.menu.count, 1);
        for (let index = 0; index < visibleRows; ++index) {
            const item = root.menu.itemAt(index);
            if (item)
                measured += item.implicitHeight;
        }
        return Math.max(1, Math.ceil(measured));
    }

    // The header does not scroll: it remains beside Flickable.contentItem.
    // During the attached fall it follows the row carrier's hidden distance,
    // so the complete card emerges as one block instead of leaving an
    // already-settled header above moving rows. The physically inset carrier
    // clips that shared movement at the panel seam.
    Column {
        id: pinnedHeading
        objectName: "celestina-tray-menu-heading"

        parent: root.menuViewport ? root.menuViewport.parent : null
        x: root.menuViewport ? root.menuViewport.x : 0
        y: CelestinaTheme.compMenuPadding - root.rowsCut
        width: root.menuViewport ? root.menuViewport.width : 0
        z: 10
        // This sibling does not inherit the scale installed on the viewport.
        // Draw it from the same corner with the same per-output factor.
        transformOrigin: Item.TopLeft
        scale: root.shellScale

        SoftMenuRow {
            ink: root.ink
            width: pinnedHeading.width
            header: true
            actionable: false
            text: root.title
            subtitle: qsTr("%n acción(es)", "", root.entries.length)
            iconName: "app-window"
            fallbackIcon: "app-window"
        }

        Item {
            width: 1
            height: root.headerBodyGap
        }
    }

    Binding {
        target: root.menu
        property: "topPadding"
        // Padding belongs to the viewport's unscaled coordinates, while the
        // sibling heading states the same output factor itself.
        value: CelestinaTheme.compMenuPadding
               + Math.round(pinnedHeading.height * root.shellScale)
    }

    Binding {
        target: root.menuViewport
        property: "clip"
        value: true
        when: root.menuViewport !== null
    }

    readonly property var displayEntries: {
        const built = [];
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
