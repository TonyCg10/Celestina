import QtQuick
import org.celestina.hematita 1.0

// One channel: the word for it, its value with the unit, the session's
// extremes, and the kernel's limit when it has one. Content family: a row in
// a grouped card, hover only, no selection — there is nothing to act on.
//
// It takes no focus of its own. Forty-six rows that each answered Tab would
// be forty-six stops between the section strip and whatever follows, and a
// row focused inside a scrolling surface that has no current item is a focus
// nobody can scroll to. The page is the one stop and its arrows move by card;
// each row still names itself to a screen reader, which is what AT reads as
// it walks the list.
Item {
    id: row

    required property string label
    required property string valueText
    required property string extremesText
    required property string limitText
    required property string load

    implicitHeight: CelestinaTheme.rowHeight
    activeFocusOnTab: false

    Accessible.role: Accessible.ListItem
    Accessible.name: row.label + ", " + row.valueText + ", " + row.extremesText
                     + (row.limitText.length > 0 ? ", " + row.limitText : "")

    Row {
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceLg
        anchors.rightMargin: CelestinaTheme.spaceLg
        spacing: CelestinaTheme.spaceMd

        Column {
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - value.width - parent.spacing
            Text {
                text: row.label
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                elide: Text.ElideRight
                width: parent.width
            }
            Text {
                text: row.extremesText + (row.limitText.length > 0 ? "   " + row.limitText : "")
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.features: CelestinaTheme.fontFeaturesTabular
                elide: Text.ElideRight
                width: parent.width
            }
        }

        Text {
            id: value
            anchors.verticalCenter: parent.verticalCenter
            text: row.valueText
            color: row.load === "critical" ? CelestinaTheme.danger
                 : row.load === "elevated" ? CelestinaTheme.warning
                 : CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            font.features: CelestinaTheme.fontFeaturesTabular
        }
    }
}
