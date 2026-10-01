import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// ─── CelestinaTopBar ──────────────────────────────────────────────────────────
// The one fixed bar an application wears, spanning the whole window above its
// sidebar and content. It is window chrome, not a floating layer: canvas fill,
// no card, no glass, `topBarHeight` tall. Three slots, left to right:
//
//   leading   navigation — a CelestinaSegmentedControl, a back button, or nothing
//   title     the page, left-aligned right after `leading`, eliding; never
//             centred, never uppercased; `subtitle` beneath it in textMuted
//   trailing  icon-only actions, right-aligned, spaceXs apart
//
// `trailing` holds CelestinaIconButton children only. A text button in it
// fails the style guard: on a desktop bar the verbs are glyphs and their names
// travel through helpText; words belong to dialogs. The bar takes no keyboard
// focus of its own; each button and the segmented control keep theirs, in
// reading order.
// ──────────────────────────────────────────────────────────────────────────────
Pane {
    id: bar

    property string title: ""
    property string subtitle: ""
    property alias leadingData: leadingSlot.data
    property alias trailingData: trailingSlot.data

    implicitHeight: CelestinaTheme.topBarHeight
    padding: 0
    leftPadding: CelestinaTheme.windowMargin
    rightPadding: CelestinaTheme.windowMargin
    activeFocusOnTab: false

    Accessible.role: Accessible.ToolBar
    Accessible.name: bar.title

    background: Rectangle {
        color: CelestinaTheme.canvas
    }

    contentItem: RowLayout {
        spacing: CelestinaTheme.spaceMd

        Row {
            id: leadingSlot
            objectName: "topBarLeading"
            // An empty slot adds no spacing before the title.
            visible: children.length > 0
            Layout.alignment: Qt.AlignVCenter
            spacing: CelestinaTheme.spaceXs
        }

        Column {
            objectName: "topBarTitles"
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
            spacing: 0

            Text {
                objectName: "topBarTitle"
                width: parent.width
                visible: bar.title.length > 0
                text: bar.title
                textFormat: Text.PlainText
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignLeft
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
                color: CelestinaTheme.text
            }

            Text {
                objectName: "topBarSubtitle"
                width: parent.width
                visible: bar.subtitle.length > 0
                text: bar.subtitle
                textFormat: Text.PlainText
                elide: Text.ElideRight
                horizontalAlignment: Text.AlignLeft
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontRowSecondary
                color: CelestinaTheme.textMuted
            }
        }

        Row {
            id: trailingSlot
            objectName: "topBarTrailing"
            Layout.alignment: Qt.AlignVCenter
            spacing: CelestinaTheme.spaceXs
        }
    }
}
