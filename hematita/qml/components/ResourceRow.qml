import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One resource in the side list: its name, its current value and a sparkline.
// Selecting it puts its detail on the right. Content family: the row paints
// the accent ramp, never the grey lift.
AbstractButton {
    id: row

    required property string name
    required property string value
    required property var series
    required property string load
    required property bool selected

    implicitHeight: CelestinaTheme.rowHeightLg
    hoverEnabled: true
    focusPolicy: Qt.TabFocus

    Accessible.role: Accessible.ListItem
    Accessible.name: row.name + ", " + row.value
    Accessible.selected: row.selected

    background: Rectangle {
        radius: CelestinaTheme.radiusSm
        color: row.selected
               ? CelestinaTheme.surfaceSelected
               : row.down
                 ? CelestinaTheme.pressedWash
                 : row.hovered
                   ? CelestinaTheme.contentHover
                   : CelestinaTheme.clear
    }

    CelestinaFocusRing {
        target: row
        cornerRadius: CelestinaTheme.radiusSm
        shown: row.visualFocus
    }

    // An Item rather than a Row: a positioner refuses anchored children, and
    // both halves want the row's vertical centre.
    contentItem: Item {
        id: content

        Column {
            id: labels
            anchors.left: content.left
            anchors.leftMargin: CelestinaTheme.spaceMd
            anchors.right: sparkline.left
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: content.verticalCenter
            spacing: CelestinaTheme.spaceXs

            Text {
                text: row.name
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: CelestinaTheme.weightDemiBold
                elide: Text.ElideRight
                width: labels.width
            }
            Text {
                text: row.value
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                font.features: CelestinaTheme.fontFeaturesTabular
                elide: Text.ElideRight
                width: labels.width
            }
        }

        HistoryGraph {
            id: sparkline
            width: CelestinaTheme.controlHeightXl * 2
            height: CelestinaTheme.controlHeightSm
            anchors.right: content.right
            anchors.rightMargin: CelestinaTheme.spaceMd
            anchors.verticalCenter: content.verticalCenter
            series: row.series
            load: row.load
        }
    }
}
