import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// One conversation of the list: who, the last message, and how many wait.
// A button to the keyboard and to assistive technology as much as to the
// pointer: Tab reaches it, Space, Enter or Return opens it, and its name
// says who, what and how many are unread.
AbstractButton {
    id: root

    required property string label
    required property string snippet
    required property int unread
    signal openRequested

    implicitHeight: 56
    hoverEnabled: true
    focusPolicy: Qt.StrongFocus
    padding: 0

    Accessible.role: Accessible.Button
    // Joined rather than formatted: the label and snippet are the peer's
    // text, and a `%1` inside them must stay text.
    Accessible.name: [root.label, root.snippet]
                     .concat(root.unread > 0 ? [qsTr("%n sin leer", "", root.unread)] : [])
                     .join(", ")

    onClicked: root.openRequested()
    Keys.onReturnPressed: root.clicked()
    Keys.onEnterPressed: root.clicked()

    background: CelestinaRowHighlight {
        family: CelestinaRowHighlight.Content
        hovered: root.hovered
        pressed: root.down
    }

    CelestinaFocusRing {
        target: root
        cornerRadius: CelestinaTheme.radiusSm
        shown: root.visualFocus
    }

    contentItem: RowLayout {
        spacing: CelestinaTheme.spaceSm

        Column {
            Layout.fillWidth: true
            Layout.leftMargin: CelestinaTheme.spaceMd
            spacing: 2

            Text {
                // Peer-supplied text: never interpreted as markup.
                textFormat: Text.PlainText
                width: parent.width
                text: root.label
                color: CelestinaTheme.text
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowTitle
                font.weight: root.unread > 0 ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
                elide: Text.ElideRight
                Accessible.ignored: true
            }

            Text {
                textFormat: Text.PlainText
                width: parent.width
                text: root.snippet
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideRight
                Accessible.ignored: true
            }
        }

        Rectangle {
            visible: root.unread > 0
            Layout.rightMargin: CelestinaTheme.spaceMd
            Layout.preferredWidth: Math.max(height, count.implicitWidth + CelestinaTheme.spaceSm)
            Layout.preferredHeight: CelestinaTheme.compStatusIndicatorSize * 2
            radius: height / 2
            color: CelestinaTheme.accent
            Accessible.ignored: true

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
}
