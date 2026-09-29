pragma ComponentBehavior: Bound

import QtQuick
import org.celestina.siderita 1.0

    // ── Compress (choose the archive's name and container) ────────────
    // Takes the paths the view already decided on and asks the one thing the
    // domain cannot know: what the archive will be called and in which
    // container. The suggested name is composed by the controller from the
    // bytes of the selection and of the folder, so no path is joined here.
CelestinaModalLayer {
    id: compress
    // Required rather than typed: the tests hand in stubs with the same
    // members, which a `SideritaController` or `Item` type would refuse.
    required property var controller
    // The surface the card sizes itself against, and gives focus back to.
    required property var owner
    // What the glass samples behind the card (the main panel).
    required property Item backdrop
    anchors.fill: parent
    z: 60
    onDismissRequested: compress.dismiss()

    // The entries to compress, exactly as the view handed them over.
    property var targets: []
    // The chosen container, under the same token the domain parses.
    property string format: "zip"

    readonly property var formats: [
        { token: "zip", label: "ZIP" },
        { token: "tar.gz", label: "TAR.GZ" }
    ]

    function openFor(paths) {
        if (!paths || paths.length === 0)
            return
        compress.targets = paths
        compress.format = "zip"
        compress.offered = ""
        nameField.text = ""
        compress.shown = true
        compress.refreshName()
        nameField.forceActiveFocus()
    }
    // The suggested name depends on the container, so it is asked for again on
    // every change — unless the person has typed their own, which is never
    // overwritten. The controller composes it off the Qt thread (whether a name
    // is taken is a question for the disk) and answers through
    // `archiveSuggestion`; `offered` is the suggestion the field last received.
    property string offered: ""
    function refreshName() {
        compress.controller.suggestArchiveName(compress.targets, compress.format)
    }
    function chooseFormat(token) {
        if (compress.format === token)
            return
        compress.format = token
        compress.refreshName()
    }
    Connections {
        target: compress.controller
        function onArchiveSuggestionChanged() {
            if (!compress.shown)
                return
            const suggestion = compress.controller.archiveSuggestion
            // Each request clears the suggestion first, so an identical answer
            // still arrives as a change; the cleared state itself is not one.
            if (suggestion.length === 0)
                return
            if (nameField.text === compress.offered || nameField.text.length === 0) {
                nameField.text = suggestion
                nameField.selectAll()
            }
            compress.offered = suggestion
        }
    }
    function dismiss() {
        compress.shown = false
        compress.targets = []
        nameField.text = ""
        compress.owner.focusView()
    }
    function confirm() {
        const value = nameField.text
        if (value.length === 0 || compress.targets.length === 0) {
            compress.dismiss()
            return
        }
        compress.controller.compressKeys(compress.targets, value, compress.format)
        compress.dismiss()
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(420, compress.owner.width - 48)
        height: 196
        backdropSource: compress.backdrop
        // A modal like every other: assistive technology hears a dialog, named
        // by its heading.
        Accessible.role: Accessible.Dialog
        Accessible.name: compressHeading.text

        // Swallow clicks so they never reach the dismiss backdrop.
        MouseArea { anchors.fill: parent }

        Text {
            id: compressHeading
            x: 18
            y: 16
            text: compress.targets.length > 1
                  ? "Comprimir " + compress.targets.length + " elementos"
                  : "Comprimir"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
        }

        CelestinaTextField {
            id: nameField
            x: 18
            y: compressHeading.y + compressHeading.height + 12
            width: parent.width - 36
            onAccepted: compress.confirm()
            Keys.onPressed: function(event) {
                if (event.key === Qt.Key_Escape) {
                    compress.dismiss()
                    event.accepted = true
                }
            }
        }

        Row {
            id: formatRow
            x: 18
            y: nameField.y + nameField.height + 12
            spacing: 8

            Repeater {
                model: compress.formats
                CelestinaButton {
                    required property var modelData
                    text: modelData.label
                    role: compress.format === modelData.token
                          ? CelestinaButton.Primary : CelestinaButton.Tonal
                    onClicked: compress.chooseFormat(modelData.token)
                }
            }
        }

        Row {
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: "Cancelar"
                onClicked: compress.dismiss()
            }
            CelestinaButton {
                text: "Comprimir"
                role: CelestinaButton.Primary
                enabled: nameField.text.length > 0
                onClicked: compress.confirm()
            }
        }
    }
}
