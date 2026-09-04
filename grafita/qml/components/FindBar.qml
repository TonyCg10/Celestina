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
        root.session.setSearch(patternField.text, caseToggle.checked, wordToggle.checked)
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

            // The two search modifiers are toggles: the shared button wears
            // Selected while checked, and the state is said in words to
            // assistive technology.
            CelestinaIconButton {
                id: caseToggle
                anchors.verticalCenter: parent.verticalCenter
                iconName: "case-sensitive"
                checkable: true
                helpText: qsTr("Distinguir mayúsculas")
                onToggled: root.refreshSearch()

                Accessible.role: Accessible.CheckBox
                Accessible.name: "Distinguir mayúsculas"
                Accessible.checked: caseToggle.checked
            }

            CelestinaIconButton {
                id: wordToggle
                anchors.verticalCenter: parent.verticalCenter
                iconName: "whole-word"
                checkable: true
                helpText: "Solo palabras completas"
                onToggled: root.refreshSearch()

                Accessible.role: Accessible.CheckBox
                Accessible.name: "Solo palabras completas"
                Accessible.checked: wordToggle.checked
            }

            CelestinaCapsule {
                anchors.verticalCenter: parent.verticalCenter

                CelestinaIconButton {
                    iconName: "chevron-up"
                    role: CelestinaButton.Ghost
                    helpText: "Anterior (Shift+F3)"
                    enabled: root.session.searchMatches > 0
                    onClicked: root.session.findPrevious()
                }

                CelestinaIconButton {
                    iconName: "chevron-down"
                    role: CelestinaButton.Ghost
                    helpText: "Siguiente (F3)"
                    enabled: root.session.searchMatches > 0
                    onClicked: root.session.findNext()
                }
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

            // Square whichever way it is toggled, so the bar never resizes
            // under the pointer that just clicked it. The window may also
            // open the replace row (Ctrl+H), so the state is bound both ways.
            CelestinaIconButton {
                id: replaceToggle
                anchors.verticalCenter: parent.verticalCenter
                iconName: "replace"
                checkable: true
                checked: root.replacing
                helpText: root.replacing ? "Ocultar reemplazo" : "Mostrar reemplazo"
                onToggled: root.replacing = replaceToggle.checked

                Accessible.role: Accessible.CheckBox
                Accessible.name: "Mostrar reemplazo"
                Accessible.checked: root.replacing
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

            CelestinaIconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: "replace"
                helpText: "Reemplazar esta coincidencia"
                enabled: root.session.searchIndex >= 0
                onClicked: root.session.replaceCurrent(replacementField.text)
            }

            CelestinaIconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: "replace-all"
                helpText: "Reemplazar todas"
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

            CelestinaIconButton {
                anchors.verticalCenter: parent.verticalCenter
                iconName: "corner-down-left"
                helpText: qsTr("Ir a la línea")
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
