import QtQuick
import org.celestina.fluorita 1.0

// A still, drawn by the toolkit.
//
// There is no media backend behind this: `Image` is Qt reading the file, which
// is the whole point — looking at a photograph must not start a decoder. The
// budget was already checked in Rust before the source was ever set, so what
// arrives here is either safe to decode or empty.
Item {
    id: view

    required property string source
    // What the window may show at most. A scaled read of a large photograph is
    // cheap where the format allows it, and never larger than the surface.
    readonly property int decodeCap: Math.max(1, Math.ceil(
        Math.max(view.width, view.height) * Screen.devicePixelRatio))

    Accessible.role: Accessible.Graphic
    Accessible.name: qsTr("Imagen")

    Image {
        id: picture

        anchors.fill: parent
        source: view.source
        // Decoding on the GUI thread would freeze the window on a large file.
        asynchronous: true
        // Honour the camera's orientation; a portrait photograph that arrives
        // sideways is the classic sign this was forgotten.
        autoTransform: true
        fillMode: Image.PreserveAspectFit
        // Cap what is decoded rather than what is drawn: `sourceSize` is what
        // makes the reader do a scaled read instead of allocating the full
        // surface first.
        sourceSize.width: view.decodeCap
        sourceSize.height: view.decodeCap
        // A still has no motion of its own; nothing here animates, so there is
        // nothing for reduced motion to turn off.
        cache: false
        visible: picture.status === Image.Ready
    }

    // Loading and failure are states, not blank space.
    CelestinaSectionLabel {
        anchors.centerIn: parent
        visible: picture.status === Image.Loading
        text: qsTr("Cargando…")
    }

    Text {
        anchors.centerIn: parent
        width: Math.min(parent.width - CelestinaTheme.spaceLg * 2, 420)
        visible: picture.status === Image.Error
        text: qsTr("El sistema no pudo decodificar esta imagen")
        color: CelestinaTheme.danger
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: CelestinaTheme.fontBody
        wrapMode: Text.WordWrap
        horizontalAlignment: Text.AlignHCenter
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
}
