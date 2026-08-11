// PANEL-1. A real Qt Quick menu in the panel's visual language.
//
// `AnchoredMenu` keeps the established Qt Quick Menu lifecycle: Escape,
// arrows, Enter, focus and outside-click dismissal. This component changes
// only its visual field. The old card plate is hidden; one transparent shared
// glass card and its denser internal sections replace independent row pills.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick

AnchoredMenu {
    id: root

    property bool compositorBlurAvailable: false
    property alias glassRects: field.glassRects
    property alias glassRegions: field.glassRegions
    readonly property BackdropInk ink: backdropInk
    // Kept at zero by default so each menu chooses its own information
    // density. Connectivity and tray menus opt into a three-part rhythm:
    // transparent space after each row, a larger break after the header card,
    // and vertical air inside each body row. Qt's real Menu remains the
    // stacking and keyboard owner; SoftMenuRow owns the visible gaps because
    // Control.spacing does not space Menu delegates.
    property int itemSpacing: 0
    property int headerBodyGap: 0
    property int rowVerticalInset: 0
    readonly property int preferredWidth: CelestinaTheme.compMenuWidth
                                          + CelestinaTheme.space3xl * 3
    readonly property int headerRowHeight: CelestinaTheme.rowHeight
                                            + CelestinaTheme.borderFocus
                                            + CelestinaTheme.spaceSm

    BackdropInk {
        id: backdropInk
    }

    function scheduleGlassCollection() {
        field.scheduleGlassCollection();
    }

    // Keep the established shared Menu for focus and keyboard semantics, but
    // suppress its painted plate and capture: this surface supplies the one
    // compositor-backed field itself.
    Binding {
        target: root.menu.background
        property: "visible"
        value: false
    }

    Binding {
        target: root.menu
        property: "width"
        value: root.preferredWidth
    }

    Binding {
        target: root.menu.background
        property: "captureEnabled"
        value: false
    }

    Binding {
        target: root.menu.background
        property: "elevation"
        value: 0
    }

    SoftMenuField {
        id: field

        x: root.menu.x
        y: root.menu.y
        width: root.contentWidth
        height: root.cardHeight
        reducedMotion: root.reducedMotion
        ink: root.ink
        animateReveal: false
        compositorBlurAvailable: root.compositorBlurAvailable

        // The first non-focusable row paints the heading section. The body is
        // the second continuous group behind the remaining rows; no row adds a
        // compositor sample or a resting pill of its own.
        MenuSection {
            anchors.leftMargin: CelestinaTheme.compMenuPadding
            anchors.rightMargin: CelestinaTheme.compMenuPadding
            anchors.topMargin: CelestinaTheme.compMenuPadding
                               + root.headerRowHeight
                               + root.headerBodyGap
            anchors.bottomMargin: CelestinaTheme.compMenuPadding
            ink: root.ink
        }
    }

    Connections {
        target: root.menu

        function onOpened() {
            field.reveal();
        }

        function onCountChanged() {
            field.scheduleGlassCollection();
        }
    }
}
