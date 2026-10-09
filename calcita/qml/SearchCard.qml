import QtQuick
import org.celestina.calcita 1.0

// The search card under the bar: the field, the hit counter «n de N» and the
// previous, next and close buttons. Enter goes to the next hit, Shift+Enter
// to the previous one, Escape closes. It only reports; the window walks the
// hits through its `CalcitaDocument` and QtPdf's search model.
Item {
    id: card

    // The hits QtPdf found and the current one (0-based, -1 for none).
    property int hitCount: 0
    property int currentHit: -1
    // There is a query, so «Sin resultados» can be said.
    property bool searching: false
    readonly property alias text: searchField.text

    // The query, once typing has paused for 150 ms.
    signal queryEdited(string text)
    signal nextRequested()
    signal previousRequested()
    signal closeRequested()

    readonly property string counterText: card.hitCount > 0
                                          ? qsTr("%1 de %2").arg(Math.max(card.currentHit, 0) + 1).arg(card.hitCount)
                                          : (card.searching ? qsTr("Sin resultados") : "")

    // Closing: a query still waiting for the pause is dropped with the text.
    function reset() {
        typing.stop()
        searchField.clear()
    }

    // Ctrl+F: the field takes the focus with its text selected.
    function focusField() {
        searchField.forceActiveFocus()
        searchField.selectAll()
    }

    objectName: "searchCard"
    implicitWidth: 380
    implicitHeight: row.implicitHeight + CelestinaTheme.spaceXs * 2

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Elevated
        radiusOverride: CelestinaTheme.radiusPill
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceXs

        // Typing settles for a moment before QtPdf searches again.
        Timer {
            id: typing
            interval: 150
            onTriggered: card.queryEdited(searchField.text)
        }

        CelestinaTextField {
            id: searchField
            objectName: "searchField"
            anchors.verticalCenter: parent.verticalCenter
            width: card.width - counter.width - previousButton.width - nextButton.width
                   - closeButton.width - row.spacing * 4 - CelestinaTheme.spaceXs * 2
            height: CelestinaTheme.controlHeightSm
            shape: CelestinaTextField.Search
            placeholderText: qsTr("Buscar en el documento")
            Accessible.name: qsTr("Buscar")
            onTextEdited: typing.restart()
            Keys.onReturnPressed: event => {
                if (event.modifiers & Qt.ShiftModifier)
                    card.previousRequested()
                else
                    card.nextRequested()
            }
            Keys.onEnterPressed: event => {
                if (event.modifiers & Qt.ShiftModifier)
                    card.previousRequested()
                else
                    card.nextRequested()
            }
            Keys.onEscapePressed: card.closeRequested()
        }

        Text {
            id: counter
            objectName: "hitCounter"
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(implicitWidth, 64)
            horizontalAlignment: Text.AlignHCenter
            text: card.counterText
            color: CelestinaTheme.textMuted
            font.pixelSize: CelestinaTheme.fontCaption
            font.features: CelestinaTheme.fontFeaturesTabular
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        CelestinaIconButton {
            id: previousButton
            objectName: "previousHitButton"
            anchors.verticalCenter: parent.verticalCenter
            role: CelestinaButton.Ghost
            iconName: "chevron-up"
            helpText: qsTr("Resultado anterior")
            enabled: card.hitCount > 0
            onClicked: card.previousRequested()
        }

        CelestinaIconButton {
            id: nextButton
            objectName: "nextHitButton"
            anchors.verticalCenter: parent.verticalCenter
            role: CelestinaButton.Ghost
            iconName: "chevron-down"
            helpText: qsTr("Resultado siguiente")
            enabled: card.hitCount > 0
            onClicked: card.nextRequested()
        }

        CelestinaIconButton {
            id: closeButton
            objectName: "closeSearchButton"
            anchors.verticalCenter: parent.verticalCenter
            role: CelestinaButton.Ghost
            iconName: "x"
            helpText: qsTr("Cerrar la búsqueda")
            onClicked: card.closeRequested()
        }
    }
}
