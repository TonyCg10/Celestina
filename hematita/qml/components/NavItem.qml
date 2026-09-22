import QtQuick
import QtQuick.Controls
import org.celestina.hematita 1.0

// One destination in the strip: a glyph over its word. The current one wears a
// lighter pill; the others paint nothing at rest and the suite's hover fill
// under the pointer. It is a radio button for the keyboard and the screen
// reader, and the strip decides which one is checked.
AbstractButton {
    id: item

    required property string iconName
    required property string label
    required property bool current

    implicitWidth: Math.max(CelestinaTheme.controlHeightXl * 2, column.implicitWidth + CelestinaTheme.spaceXl * 2)
    implicitHeight: CelestinaTheme.controlHeightXl + CelestinaTheme.spaceSm

    checkable: true
    checked: item.current
    autoExclusive: true
    hoverEnabled: true
    focusPolicy: Qt.TabFocus

    Accessible.role: Accessible.PageTab
    Accessible.name: item.label
    Accessible.checked: item.current

    background: Rectangle {
        radius: CelestinaTheme.radiusPill
        color: item.current
               ? CelestinaTheme.elevated
               : item.down
                 ? CelestinaTheme.pressedWash
                 : item.hovered
                   ? CelestinaTheme.surfaceHover
                   : CelestinaTheme.clear
        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion ? 0 : CelestinaTheme.motionFast
            }
        }
    }

    // The ring anchors to its target, so it must be the target's child, not
    // the background's: anchors reach only a parent or a sibling.
    CelestinaFocusRing {
        target: item
        cornerRadius: item.height / 2
        shown: item.visualFocus
    }

    contentItem: Column {
        id: column
        spacing: CelestinaTheme.spaceXs
        anchors.centerIn: parent

        CelestinaIcon {
            anchors.horizontalCenter: parent.horizontalCenter
            name: item.iconName
            width: CelestinaTheme.iconMd
            height: width
            tone: item.current ? CelestinaIcon.Primary : CelestinaIcon.Secondary
        }

        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: item.label
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody
            font.weight: item.current ? CelestinaTheme.weightDemiBold : CelestinaTheme.weightRegular
            color: item.current ? CelestinaTheme.text : CelestinaTheme.textMuted
        }
    }
}
