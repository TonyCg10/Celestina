import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// One conversation of the list: who, the last message, and how many wait.
Item {
    id: root

    required property string label
    required property string snippet
    required property string unread
    signal openRequested

    height: 56

    CelestinaRowHighlight {
        anchors.fill: parent
        family: CelestinaRowHighlight.Content
        hovered: mouse.containsMouse
        pressed: mouse.pressed
    }

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceMd
        anchors.rightMargin: CelestinaTheme.spaceMd
        spacing: CelestinaTheme.spaceSm

        Column {
            Layout.fillWidth: true
            spacing: 2

            Text {
                // Peer-supplied text: never interpreted as markup.
                textFormat: Text.PlainText
                width: parent.width
                text: root.label
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: root.unread !== "0" ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                elide: Text.ElideRight
            }

            Text {
                textFormat: Text.PlainText
                width: parent.width
                text: root.snippet
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
            }
        }

        Rectangle {
            visible: root.unread !== "0"
            width: Math.max(height, count.implicitWidth + CelestinaTheme.spaceSm)
            height: CelestinaTheme.compStatusIndicatorSize * 2
            radius: height / 2
            color: CelestinaTheme.accent

            Text {
                id: count
                anchors.centerIn: parent
                text: root.unread
                color: CelestinaTheme.accentInk
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontMini
                font.weight: CelestinaTheme.weightDemiBold
            }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        onClicked: root.openRequested()
    }
}
