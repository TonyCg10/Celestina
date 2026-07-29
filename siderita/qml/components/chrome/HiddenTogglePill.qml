import QtQuick
import org.celestina.siderita 1.0

// The one visual contract for revealing dotfiles in every Siderita browser.
FloatingButton {
    id: control

    required property bool toggleChecked
    property real textScale: 1.0

    signal toggleRequested

    text: "Ocultos"
    helpText: toggleChecked
              ? "Ocultar elementos ocultos"
              : "Mostrar elementos ocultos"
    active: toggleChecked
    font.pixelSize: Math.round(CelestinaTheme.fontMini * textScale)
    Accessible.name: toggleChecked
                     ? "Ocultar elementos ocultos"
                     : "Mostrar elementos ocultos"
    Accessible.checkable: true
    Accessible.checked: toggleChecked
    onClicked: toggleRequested()
}
