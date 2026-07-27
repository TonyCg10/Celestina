import QtQuick

// ─── ListSection ──────────────────────────────────────────────────────────────
// The One UI signature: a grouped rounded card ("focus block"). Rows are grouped
// into one 26-radius card floating on the window background — separation by
// grouping, not by hairlines between cards (DESIGN §2/§6.8). An optional section
// header (uppercase caption) sits above. The consumer's rows are the default
// children; they stack inside the card.
// ──────────────────────────────────────────────────────────────────────────────
Column {
    id: section

    property string title: ""
    // Consumer rows land in the card's inner column, not directly on this root.
    default property alias rows: rowHolder.data

    spacing: CelestinaTheme.spaceSm

    Text {
        visible: section.title.length > 0
        text: section.title
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontMini
        font.letterSpacing: 1.4
        font.weight: CelestinaTheme.weightDemiBold
        leftPadding: CelestinaTheme.spaceMd
    }

    Rectangle {
        width: section.width
        implicitHeight: rowHolder.implicitHeight + CelestinaTheme.spaceXs * 2
        height: implicitHeight
        radius: CelestinaTheme.radiusLg
        color: CelestinaTheme.card

        Column {
            id: rowHolder
            width: parent.width
            y: CelestinaTheme.spaceXs
        }
    }
}
