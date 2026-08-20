pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.grafita 1.0

// Which encoding a document is read as. Nothing in a `latin-1` note or an
// unmarked UTF-16 file says which one it is, so this is the only place that
// answer can come from. Choosing one re-reads the file; the document core
// refuses the choice if that encoding would not write the file back byte for
// byte, so a wrong pick is a refusal and never a silent rewrite.
CelestinaModalLayer {
    id: chooser

    required property var session
    required property Item backdrop

    z: 92
    shown: chooser.session.encodingPrompt
    dismissOnEscape: true
    dismissOnOutsideClick: true
    onDismissRequested: chooser.session.cancelEncodingChooser()

    onShownChanged: if (shown) {
        list.currentIndex = chooser.session.encodingIndex >= 0
                            ? chooser.session.encodingIndex : 0
        list.positionViewAtIndex(list.currentIndex, ListView.Center)
        list.forceActiveFocus()
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, chooser.width - CelestinaTheme.space3xl)
        height: Math.min(460, chooser.height - CelestinaTheme.space3xl)
        backdropSource: chooser.backdrop

        Accessible.role: Accessible.Dialog
        Accessible.name: "Document encoding"

        MouseArea { anchors.fill: parent }

        Text {
            id: heading
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: parent.top
            anchors.margins: CelestinaTheme.spaceLg
            wrapMode: Text.WordWrap
            text: chooser.session.active
                  ? "Leer «" + chooser.session.name + "» como:"
                  : "Leer este archivo como:"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontBody

            Accessible.role: Accessible.StaticText
            Accessible.name: heading.text
        }

        ListView {
            id: list
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: heading.bottom
            anchors.bottom: buttons.top
            anchors.margins: CelestinaTheme.spaceLg
            clip: true
            model: chooser.session.encodingNames
            currentIndex: 0
            keyNavigationEnabled: true

            Accessible.role: Accessible.List
            Accessible.name: "Encodings"

            delegate: Item {
                required property int index
                required property string modelData

                width: list.width
                height: label.implicitHeight + CelestinaTheme.spaceMd

                id: row

                // Shown rather than tinted to nothing: the selected row is the
                // only one that carries a surface at all.
                Rectangle {
                    anchors.fill: row
                    visible: row.ListView.isCurrentItem
                    radius: CelestinaTheme.radiusSm
                    color: CelestinaTheme.surfaceHover
                }

                Text {
                    id: label
                    anchors.left: row.left
                    anchors.right: row.right
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.verticalCenter: row.verticalCenter
                    elide: Text.ElideRight
                    // The one it is read as now carries the mark, so choosing
                    // is a change from something rather than a guess.
                    text: (row.index === chooser.session.encodingIndex
                           ? "• " : "") + row.modelData
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody

                    Accessible.role: Accessible.ListItem
                    Accessible.name: row.modelData
                }

                MouseArea {
                    anchors.fill: row
                    onClicked: {
                        list.currentIndex = row.index
                        chooser.apply()
                    }
                }
            }

            Keys.onReturnPressed: chooser.apply()
            Keys.onEnterPressed: chooser.apply()
        }

        Row {
            id: buttons
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: CelestinaTheme.spaceLg
            spacing: CelestinaTheme.spaceSm

            CelestinaButton {
                text: "Cancelar"
                onClicked: chooser.session.cancelEncodingChooser()
            }
            CelestinaButton {
                text: "Aplicar"
                role: CelestinaButton.Primary
                onClicked: chooser.apply()
            }
        }
    }

    function apply() {
        chooser.session.chooseEncoding(list.currentIndex)
    }
}
