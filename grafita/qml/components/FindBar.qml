import QtQuick
import org.celestina.grafita 1.0

// Find, replace and go-to-line, in one strip above the document.
//
// It reports what it is doing rather than only acting: the count says how many
// matches there are and which one you are on, so "no encontrado" is a stated
// result and not a search that silently did nothing.
Item {
    id: root

    required property var session
    // Whether the strip is showing. The window owns this, so a shortcut can
    // open it and Escape can close it from anywhere.
    property bool shown: false
    // Replace and go-to-line stay folded away until asked for: finding is the
    // common case and deserves the smaller surface.
    property bool replacing: false

    signal dismissed

    visible: shown
    implicitHeight: shown ? layout.implicitHeight + CelestinaTheme.spaceMd * 2 : 0

    // Called by the window when the strip opens.
    function takeFocus() {
        patternField.forceActiveFocus()
        patternField.selectAll()
    }

    function refreshSearch() {
        root.session.setSearch(patternField.text, caseToggle.on, wordToggle.on)
    }

    onShownChanged: if (!shown) root.session.setSearch("", false, false)

    Rectangle {
        anchors.fill: parent
        color: CelestinaTheme.surface
        border.width: CelestinaTheme.borderHairline
        border.color: CelestinaTheme.divider
    }

    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            root.dismissed()
            event.accepted = true
        }
    }

    Column {
        id: layout
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        anchors.margins: CelestinaTheme.spaceMd
        spacing: CelestinaTheme.spaceSm

        // ── Find ──────────────────────────────────────────────────────────
        Row {
            spacing: CelestinaTheme.spaceSm

            CelestinaTextField {
                id: patternField
                width: 240
                placeholderText: "Buscar"
                onTextChanged: root.refreshSearch()
                Keys.onReturnPressed: function(event) {
                    if (event.modifiers & Qt.ShiftModifier)
                        root.session.findPrevious()
                    else
                        root.session.findNext()
                    event.accepted = true
                }

                Accessible.role: Accessible.EditableText
                Accessible.name: "Buscar"
            }

            CelestinaButton {
                id: caseToggle
                // A plain toggle: the shared button carries no checked state, so
                // the pressed look is expressed through its role and the state
                // is said in words to assistive technology.
                property bool on: false
                text: "Aa"
                role: caseToggle.on ? CelestinaButton.Primary : CelestinaButton.Tonal
                onClicked: { caseToggle.on = !caseToggle.on; root.refreshSearch() }

                Accessible.role: Accessible.CheckBox
                Accessible.name: "Distinguir mayúsculas"
                Accessible.checked: caseToggle.on
            }

            CelestinaButton {
                id: wordToggle
                property bool on: false
                text: "|ab|"
                role: wordToggle.on ? CelestinaButton.Primary : CelestinaButton.Tonal
                onClicked: { wordToggle.on = !wordToggle.on; root.refreshSearch() }

                Accessible.role: Accessible.CheckBox
                Accessible.name: "Solo palabras completas"
                Accessible.checked: wordToggle.on
            }

            CelestinaButton {
                text: "Anterior"
                enabled: root.session.searchMatches > 0
                onClicked: root.session.findPrevious()
            }

            CelestinaButton {
                text: "Siguiente"
                enabled: root.session.searchMatches > 0
                onClicked: root.session.findNext()
            }

            Text {
                id: tally
                anchors.verticalCenter: parent.verticalCenter
                text: {
                    if (patternField.text.length === 0)
                        return ""
                    if (root.session.searchMatches === 0)
                        return "Sin coincidencias"
                    const position = root.session.searchIndex >= 0
                                   ? (root.session.searchIndex + 1) + " de " : ""
                    return position + root.session.searchMatches
                }
                color: root.session.searchMatches === 0 && patternField.text.length > 0
                       ? CelestinaTheme.warning : CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption

                // Colour alone must not carry "nothing found".
                Accessible.role: Accessible.StaticText
                Accessible.name: tally.text
                Accessible.ignored: tally.text.length === 0
            }

            CelestinaButton {
                text: root.replacing ? "Menos" : "Reemplazar…"
                onClicked: root.replacing = !root.replacing
            }
        }

        // ── Replace and go to line ────────────────────────────────────────
        Row {
            spacing: CelestinaTheme.spaceSm
            visible: root.replacing

            CelestinaTextField {
                id: replacementField
                width: 240
                placeholderText: "Reemplazar por"

                Accessible.role: Accessible.EditableText
                Accessible.name: "Reemplazar por"
            }

            CelestinaButton {
                text: "Reemplazar"
                enabled: root.session.searchIndex >= 0
                onClicked: root.session.replaceCurrent(replacementField.text)
            }

            CelestinaButton {
                text: "Reemplazar todo"
                enabled: root.session.searchMatches > 0
                onClicked: root.session.replaceAll(replacementField.text)
            }

            CelestinaTextField {
                id: lineField
                width: 96
                placeholderText: "Línea"
                inputMethodHints: Qt.ImhDigitsOnly
                onAccepted: root.goToTypedLine()

                Accessible.role: Accessible.EditableText
                Accessible.name: "Ir a la línea"
            }

            CelestinaButton {
                text: "Ir"
                enabled: lineField.text.length > 0
                onClicked: root.goToTypedLine()
            }
        }
    }

    function goToTypedLine() {
        const line = parseInt(lineField.text, 10)
        if (!isNaN(line) && line > 0)
            root.session.goToLine(line)
    }
}
