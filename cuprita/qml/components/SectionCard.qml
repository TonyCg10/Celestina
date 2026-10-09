import QtQuick
import org.celestina.cuprita 1.0

// One group of a page: an uppercase label, then a rounded card holding the
// group's rows. The rows stack in a column kept `spaceCardInset` inside the
// card, so a row's radiusMd plate is concentric with the card's radiusLg
// corner; each row keeps its own words and controls a further inset in, so
// they sit `spaceLg` from the card's edge.
Column {
    id: section

    required property string title
    // A command for this card is in flight: a small spinner beside the label.
    property bool working: false
    // How far inside the row column a row's words start and its controls
    // end, so both sit `spaceLg` from the card's edge.
    readonly property int rowInset: CelestinaTheme.spaceLg - CelestinaTheme.spaceCardInset
    // The rows: they land in the card's inner column.
    default property alias rows: holder.data

    spacing: CelestinaTheme.spaceSm

    Row {
        width: section.width
        spacing: CelestinaTheme.spaceSm

        CelestinaSectionLabel {
            id: label
            objectName: "sectionLabel"
            leftPadding: CelestinaTheme.spaceLg
            // The suite's eyebrow is set in capitals; the words stay the caller's.
            text: section.title.toUpperCase()
        }

        Spinner {
            objectName: "workingSpinner"
            anchors.verticalCenter: label.verticalCenter
            // As tall as the label, so showing it never moves the card.
            width: label.height
            height: label.height
            visible: section.working
        }
    }

    CelestinaSurface {
        width: section.width
        height: holder.implicitHeight + CelestinaTheme.spaceCardInset * 2
        role: CelestinaSurface.Grouped
        Accessible.ignored: true

        Column {
            id: holder
            x: CelestinaTheme.spaceCardInset
            y: CelestinaTheme.spaceCardInset
            width: parent.width - CelestinaTheme.spaceCardInset * 2
        }
    }
}
