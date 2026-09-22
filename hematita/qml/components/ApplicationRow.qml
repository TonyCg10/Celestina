pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import org.celestina.hematita 1.0

// One application in the grouped layout: its icon from the desktop's icon
// theme, its name, how many processes, and their CPU and memory together.
// The row expands and collapses its processes.
AbstractButton {
    id: row

    required property string appName
    // A theme name, or an absolute path when the `.desktop` file gave one.
    required property string iconName
    required property int count
    required property string cpu
    required property string memory
    required property bool expanded

    readonly property bool iconIsPath: row.iconName.startsWith("/")
    readonly property string countText: row.count === 1
                                        ? qsTr("1 proceso")
                                        : qsTr("%1 procesos").arg(row.count)

    implicitHeight: CelestinaTheme.rowHeight
    hoverEnabled: true
    focusPolicy: Qt.TabFocus
    checkable: true
    checked: row.expanded

    Accessible.role: Accessible.ListItem
    Accessible.name: row.appName + ", " + row.countText + ", " + row.cpu
    Accessible.checked: row.expanded

    background: CelestinaRowHighlight {
        family: CelestinaRowHighlight.Content
        hovered: row.hovered
        pressed: row.down
        selected: false
        focused: row.visualFocus
    }

    contentItem: Item {
        Row {
            id: line

            anchors.fill: parent
            anchors.leftMargin: CelestinaTheme.spaceSm
            spacing: CelestinaTheme.spaceMd

            CelestinaIcon {
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.iconSm
                height: width
                name: row.expanded ? "chevron-down" : "chevron-right"
                tone: CelestinaIcon.Secondary
            }

            // The desktop's icon for the application, through Qt's theme
            // lookup. Not a catalogue glyph: this is the application's own
            // face. When the theme has none, the catalogue's window glyph.
            Item {
                anchors.verticalCenter: parent.verticalCenter
                width: CelestinaTheme.glyphTile
                height: width

                IconImage {
                    id: themed
                    anchors.fill: parent
                    name: row.iconIsPath ? "" : row.iconName
                    source: row.iconIsPath ? "file://" + row.iconName : ""
                    sourceSize: Qt.size(themed.width, themed.height)
                    visible: themed.status === Image.Ready
                }

                CelestinaIcon {
                    anchors.fill: parent
                    visible: !themed.visible
                    name: "app-window"
                    tone: CelestinaIcon.Secondary
                }
            }

            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: Math.max(0, line.width - CelestinaTheme.glyphTile - CelestinaTheme.iconSm
                                   - line.spacing * 3 - CelestinaTheme.spaceSm - totals.width)

                Text {
                    text: row.appName
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowTitle
                    font.weight: CelestinaTheme.weightDemiBold
                    elide: Text.ElideRight
                    width: parent.width
                }

                Text {
                    text: row.countText
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontRowSecondary
                }
            }

            Text {
                id: totals
                anchors.verticalCenter: parent.verticalCenter
                text: row.cpu + "   " + row.memory
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontBody
                font.features: CelestinaTheme.fontFeaturesTabular
                rightPadding: CelestinaTheme.spaceSm
            }
        }
    }
}
