import QtQuick
import org.celestina.selenita 1.0

// One region of the window in the CUP-1-H grammar: an uppercase eyebrow, then
// a grouped card whose rows stack `spaceCardInset` inside its edge, so a
// row's plate stays concentric with the card's corner. A row keeps its words
// and controls `rowInset` further in, `spaceLg` from the card's edge.
Column {
    id: card

    required property string title
    // Where a row's words start and its controls end, inside the row column.
    readonly property int rowInset: CelestinaTheme.spaceLg - CelestinaTheme.spaceCardInset
    // The rows land in the card's inner column.
    default property alias rows: rowColumn.data

    spacing: CelestinaTheme.spaceSm
    Accessible.role: Accessible.Grouping
    Accessible.name: title

    CelestinaSectionLabel {
        objectName: "sectionLabel"
        leftPadding: CelestinaTheme.spaceLg
        // The suite sets its eyebrows in capitals; the words stay the caller's.
        text: card.title.toUpperCase()
        // The grouping reads the title; the eyebrow is not read again.
        Accessible.ignored: true
    }

    CelestinaSurface {
        width: card.width
        height: rowColumn.implicitHeight + CelestinaTheme.spaceCardInset * 2
        role: CelestinaSurface.Grouped
        Accessible.ignored: true

        Column {
            id: rowColumn
            x: CelestinaTheme.spaceCardInset
            y: CelestinaTheme.spaceCardInset
            width: parent.width - CelestinaTheme.spaceCardInset * 2
        }
    }
}
