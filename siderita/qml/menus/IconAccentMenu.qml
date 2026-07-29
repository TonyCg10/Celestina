import QtQuick
import QtQml.Models
import QtQuick.Controls
import org.celestina.siderita 1.0

// Closed, token-backed accent chooser for one entry. It intentionally offers
// names and swatches rather than a free colour picker: every result stays
// legible on the suite's dark surfaces and can be retuned centrally later.
GlassContextMenu {
    id: root

    property string currentKey: ""
    signal accentSelected(string accentKey)

    readonly property var choices: [""].concat(CelestinaTheme.iconAccentKeys)

    Connections {
        target: root
        function onAboutToShow() {
            root.currentIndex = Math.max(0, root.choices.indexOf(root.currentKey))
        }
    }

    Instantiator {
        model: root.choices

        delegate: GlassMenuItem {
            required property string modelData

            text: CelestinaTheme.iconAccentLabel(modelData)
            showSwatch: true
            automaticSwatch: modelData.length === 0
            swatchColor: CelestinaTheme.iconAccentColor(modelData)
            choice: true
            current: root.currentKey === modelData
            Accessible.name: text
            Accessible.description: current
                                    ? "Color de icono seleccionado" : ""
            onTriggered: root.accentSelected(modelData)
        }

        onObjectAdded: function(index, object) {
            root.insertItem(index, object)
        }
        onObjectRemoved: function(index, object) {
            root.removeItem(object)
        }
    }
}
