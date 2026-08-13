// The one anatomy for a card-shaped contextual surface: header, dense body
// section, dismissal, and a height that is measured, not estimated.
//
// Audio, brightness and the calendar each grew this by hand, copied loosely
// from the wallpaper gallery, and each copy drifted: heights were summed from
// constants that disagreed with what the rows actually drew (three times, in
// three different ways), one dropped the body's dense material, and every one
// repeated the Escape shortcut, the outside-click carrier and the reveal
// wiring. This file is that anatomy once. `AnchoredMenu` is its sibling for
// popup-backed row menus; the two share `AnchoredCard` underneath.
//
// Height follows the repo's own convention — `AnchoredMenu.naturalMenuHeight`
// measures its items — with the one rule that keeps measurement from becoming
// the binding loop the hand-rolled cards hit: the body is measured through
// `implicitHeight` only, which for a `Column` of fixed-implicit children never
// depends on the card's own size. Anything whose implicit height would read
// its parent's geometry does not belong in a card body.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

AnchoredCard {
    id: root

    // The heading, drawn by the shared MenuHeader.
    required property string title
    property string subtitle: ""
    property string iconName: ""
    // Compact trailing actions for the header, per the icon-first hierarchy.
    property alias headerActions: header.actions
    // What the card is about, laid out in a padded column on one dense
    // section. Children state their own implicit heights.
    default property alias body: bodyColumn.data
    property int bodySpacing: CelestinaTheme.spaceMd
    property alias compositorBlurAvailable: card.compositorBlurAvailable
    property alias glassRects: card.glassRects
    property alias glassRegions: card.glassRegions
    readonly property BackdropInk ink: backdropInk

    contentWidth: 360
    // Measured: the paddings, the header and the laid-out body, nothing else.
    contentHeight: CelestinaTheme.spaceMd * 2
                   + header.implicitHeight
                   + root.bodySpacing
                   + bodySection.implicitHeight

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        onActivated: root.dismissed()
    }

    BackdropInk {
        id: backdropInk
    }

    onReady: card.reveal()

    // The full-output carrier makes a click outside the card deterministic.
    // It is declared first so the card's own input stop remains above it.
    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: root.dismissed()

        MouseArea {
            anchors.fill: parent
            onClicked: root.dismissed()
        }
    }

    SoftOverlayCard {
        id: card

        x: root.cardX
        y: root.cardY
        width: root.contentWidth
        height: root.cardHeight
        reducedMotion: root.reducedMotion
        ink: backdropInk
        accessibleName: root.title
        attachedToTop: root.anchoredFromPanel
        openerRect: root.openerRect
        attachmentAnchorRect: root.attachmentAnchorRect
        attachmentStartY: root.attachmentStartY
        surfacePosition: Qt.point(root.cardX, root.cardY)

        Flickable {
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceMd
            contentHeight: header.implicitHeight + root.bodySpacing
                           + bodySection.implicitHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds

            MenuHeader {
                id: header

                width: parent.width
                ink: backdropInk
                title: root.title
                subtitle: root.subtitle
                iconName: root.iconName
            }

            // One dense section behind the whole body, exactly as SoftMenu
            // lays its rows on one, with the body column padded inside it so
            // content never sits flush against the material's edge.
            Item {
                id: bodySection

                anchors.top: header.bottom
                anchors.topMargin: root.bodySpacing
                width: parent.width
                implicitHeight: bodyColumn.implicitHeight
                                + CelestinaTheme.spaceMd * 2

                MenuSection {
                    ink: backdropInk
                }

                Column {
                    id: bodyColumn

                    anchors.top: parent.top
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.margins: CelestinaTheme.spaceMd
                    spacing: root.bodySpacing
                }
            }
        }
    }
}
