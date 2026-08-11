// Shell tools exposed through the panel's real contextual-menu path.
//
// This surface only asks for a capture. Niri owns the screenshot UI and save
// destination, and the panel reports only a request it could not send.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

SoftMenu {
    id: root

    signal captureRequested()

    title: qsTr("Caja de herramientas")
    itemSpacing: CelestinaTheme.spaceSm
    headerBodyGap: CelestinaTheme.spaceMd
    rowVerticalInset: CelestinaTheme.spaceXs

    // Keep one ordered stream under the real Menu owner. Later capture modes
    // can add entries without mixing static children and model delegates.
    readonly property var entries: [
        {"kind": "header", "text": qsTr("Caja de herramientas"),
         "subtitle": qsTr("Herramientas disponibles")},
        {"kind": "section", "text": qsTr("Herramientas")},
        {"kind": "capture", "text": qsTr("Capturar pantalla")}
    ]

    function requestCapture() {
        root.captureRequested();
        root.menu.close();
    }

    Instantiator {
        model: root.entries
        onObjectAdded: (index, object) => root.menu.insertItem(index, object)
        onObjectRemoved: (index, object) => root.menu.removeItem(object)

        delegate: SoftMenuRow {
            id: entry

            required property var modelData

            readonly property bool isHeader: entry.modelData.kind === "header"
            readonly property bool isSection: entry.modelData.kind === "section"
            readonly property bool isCapture: entry.modelData.kind === "capture"

            ink: root.ink
            text: entry.modelData.text
            header: entry.isHeader
            sectionLabel: entry.isSection
            subtitle: entry.isHeader ? entry.modelData.subtitle : ""
            iconName: entry.isHeader ? "toolbox"
                                      : entry.isCapture ? "scissors" : ""
            actionable: entry.isCapture
            headerTrailingGap: entry.isHeader ? root.headerBodyGap : 0
            verticalInset: root.rowVerticalInset
            trailingGap: entry.isHeader ? 0 : root.itemSpacing
            onTriggered: {
                if (entry.isCapture)
                    root.requestCapture();
            }
        }
    }
}
