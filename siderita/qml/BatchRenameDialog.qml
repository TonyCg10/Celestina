import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Batch rename ─────────────────────────────────────────────────
    // Renames a whole selection by a rule rather than one dialog at a time.
    // Everything is previewed before anything is touched: the new name is
    // shown per entry, and a name that would collide — with a sibling in the
    // batch or with a file already in the folder — is marked and the rename
    // refuses to run. The domain refuses to overwrite anyway; this just
    // means the user finds out before, not after.
Rectangle {
    id: batchRename
    property var controller
    property var owner
    anchors.fill: parent
    z: 61
    property bool shown: false
    // Fades rather than pops. Opacity only: a scale transform on a
    // glass surface desyncs its backdrop sampling (see a995619), so the
    // motion here never touches geometry.
    visible: opacity > 0.01
    opacity: shown ? 1 : 0
    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }
    color: Qt.rgba(0, 0, 0, 0.45)

    property var targets: []          // [{path, name}]

    function open(paths) {
        var list = []
        for (var i = 0; i < paths.length; i++) {
            var p = paths[i]
            var slash = p.lastIndexOf("/")
            list.push({ path: p,
                        name: slash >= 0 ? p.substring(slash + 1) : p })
        }
        targets = list
        findField.text = ""
        replaceField.text = ""
        patternField.text = ""
        startField.text = "1"
        batchRename.shown = true
        findField.forceActiveFocus()
    }
    function dismiss() {
        batchRename.shown = false
        targets = []
        owner.focusView()
    }

    // The name entry `i` would end up with. A pattern (with # for the
    // number) replaces the whole name and keeps the extension; find /
    // replace edits the name in place. An empty rule leaves it alone.
    function newNameFor(index) {
        const original = targets[index].name
        const pattern = patternField.text
        if (pattern.length > 0) {
            const start = parseInt(startField.text, 10)
            const n = (isNaN(start) ? 1 : start) + index
            const dot = original.lastIndexOf(".")
            const extension = dot > 0 ? original.substring(dot) : ""
            return pattern.replace(/#+/g, function(hashes) {
                var text = String(n)
                while (text.length < hashes.length)
                    text = "0" + text
                return text
            }) + extension
        }
        if (findField.text.length > 0)
            return original.split(findField.text).join(replaceField.text)
        return original
    }

    // Names that cannot be given: empty, path-separated, colliding with
    // another entry in the batch, or with a name already in the folder
    // that is not itself being renamed away.
    readonly property var clashes: {
        var seen = ({})
        var bad = ({})
        var keeping = ({})
        var i
        for (i = 0; i < targets.length; i++)
            keeping[targets[i].name] = true
        for (i = 0; i < targets.length; i++) {
            var name = newNameFor(i)
            if (name.length === 0 || name.indexOf("/") >= 0) {
                bad[i] = true
                continue
            }
            if (seen[name] === true) {
                bad[i] = true
                continue
            }
            seen[name] = true
            if (name !== targets[i].name
                    && controller.entryNames.indexOf(name) >= 0
                    && keeping[name] !== true)
                bad[i] = true
        }
        return bad
    }
    readonly property bool anyClash: Object.keys(clashes).length > 0
    readonly property bool anyChange: {
        for (var i = 0; i < targets.length; i++)
            if (newNameFor(i) !== targets[i].name)
                return true
        return false
    }

    function confirm() {
        if (anyClash || !anyChange)
            return
        var paths = []
        var names = []
        for (var i = 0; i < targets.length; i++) {
            var name = newNameFor(i)
            if (name === targets[i].name)
                continue
            paths.push(targets[i].path)
            names.push(name)
        }
        if (paths.length > 0)
            controller.renamePaths(paths, names)
        batchRename.dismiss()
    }

    MouseArea {
        anchors.fill: parent
        onClicked: batchRename.dismiss()
    }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape) {
            batchRename.dismiss()
            event.accepted = true
        }
    }
    focus: batchRename.shown

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(560, owner.width - 48)
        height: Math.min(460, owner.height - 64)
        backdropSource: mainPanel
        Accessible.role: Accessible.Dialog
        Accessible.name: "Renombrar en lote"

        MouseArea { anchors.fill: parent }

        Text {
            id: batchHeading
            x: 18
            y: 16
            text: "Renombrar " + batchRename.targets.length + " elementos"
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCallout
            font.weight: CelestinaTheme.weightDemiBold
        }

        Grid {
            id: batchFields
            x: 18
            y: batchHeading.y + batchHeading.height + 12
            width: parent.width - 36
            columns: 2
            columnSpacing: 10
            rowSpacing: 8

            readonly property real fieldWidth:
                    (batchFields.width - batchFields.columnSpacing) / 2

            CelestinaTextField {
                id: findField
                width: batchFields.fieldWidth
                placeholderText: "Buscar"
            }
            CelestinaTextField {
                id: replaceField
                width: batchFields.fieldWidth
                placeholderText: "Reemplazar por"
            }
            CelestinaTextField {
                id: patternField
                width: batchFields.fieldWidth
                placeholderText: "Patrón, p. ej. foto-##"
            }
            CelestinaTextField {
                id: startField
                width: batchFields.fieldWidth
                placeholderText: "Empezar en"
                text: "1"
            }
        }

        Text {
            id: batchNote
            x: 18
            y: batchFields.y + batchFields.height + 10
            width: parent.width - 36
            text: batchRename.anyClash
                  ? "Hay nombres repetidos o ya usados (marcados abajo)."
                  : "El patrón sustituye el nombre y conserva la extensión; # es el número."
            color: batchRename.anyClash ? CelestinaTheme.dangerText
                                        : CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            wrapMode: Text.Wrap
        }

        ListView {
            id: batchPreview
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: batchNote.bottom
            anchors.bottom: batchButtons.top
            anchors.leftMargin: 18
            anchors.rightMargin: 18
            anchors.topMargin: 10
            anchors.bottomMargin: 12
            clip: true
            model: batchRename.targets.length
            spacing: 2
            boundsBehavior: Flickable.StopAtBounds

            delegate: Item {
                required property int index
                width: batchPreview.width
                height: 24

                readonly property bool clashes:
                        batchRename.clashes[index] === true
                readonly property string before:
                        batchRename.targets[index].name
                readonly property string after: batchRename.newNameFor(index)

                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width * 0.44
                    text: parent.before
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideMiddle
                }
                Text {
                    x: parent.width * 0.46
                    anchors.verticalCenter: parent.verticalCenter
                    text: "→"
                    color: CelestinaTheme.textMuted
                    font.pixelSize: CelestinaTheme.fontCaption
                }
                Text {
                    x: parent.width * 0.52
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width * 0.48
                    text: parent.after
                    color: parent.clashes ? CelestinaTheme.dangerText
                           : parent.after !== parent.before ? CelestinaTheme.accent
                           : CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    elide: Text.ElideMiddle
                }
            }
        }

        Row {
            id: batchButtons
            anchors.right: parent.right
            anchors.rightMargin: 18
            anchors.bottom: parent.bottom
            anchors.bottomMargin: 16
            spacing: 8

            CelestinaButton {
                text: "Cancelar"
                onClicked: batchRename.dismiss()
            }
            CelestinaButton {
                text: "Renombrar"
                primary: true
                enabled: !batchRename.anyClash && batchRename.anyChange
                onClicked: batchRename.confirm()
            }
        }
    }
}
