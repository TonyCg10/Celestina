pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One process. Content family: the shared row highlight paints hover, press
// and selection; the cells only write text.
AbstractButton {
    id: row

    required property var columns
    required property string name
    required property string user
    required property string pid
    required property string cpu
    required property string memory
    required property string read
    required property string write
    required property bool actionable
    required property bool selected
    // Indented under its application in the grouped layout.
    property bool nested: false

    implicitHeight: CelestinaTheme.controlHeightSm
    hoverEnabled: true
    focusPolicy: Qt.TabFocus

    Accessible.role: Accessible.ListItem
    Accessible.name: row.name + ", " + row.pid + ", " + row.cpu + ", " + row.memory
    Accessible.selected: row.selected

    background: CelestinaRowHighlight {
        family: CelestinaRowHighlight.Content
        hovered: row.hovered
        pressed: row.down
        selected: row.selected
        focused: row.visualFocus
    }

    contentItem: Item {
        // A process that is not the user's own cannot be acted on; the row
        // says so by weight rather than by a sentence per line.
        opacity: row.actionable ? 1 : CelestinaTheme.mutedContentOpacity

        Row {
            anchors.fill: parent
            anchors.leftMargin: row.nested ? CelestinaTheme.space2xl : 0

            Repeater {
                model: row.columns

                Text {
                    id: cell

                    required property var modelData

                    width: cell.modelData.width
                           - (row.nested && cell.modelData.field === "name" ? CelestinaTheme.space2xl : 0)
                    height: row.height
                    leftPadding: CelestinaTheme.spaceSm
                    rightPadding: CelestinaTheme.spaceSm
                    verticalAlignment: Text.AlignVCenter
                    horizontalAlignment: cell.modelData.numeric ? Text.AlignRight : Text.AlignLeft
                    elide: Text.ElideRight
                    text: row[cell.modelData.field]
                    color: cell.modelData.field === "name" ? CelestinaTheme.text : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    font.features: CelestinaTheme.fontFeaturesTabular
                }
            }
        }
    }
}
