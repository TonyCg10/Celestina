import QtQuick
import QtQuick.Effects
import org.celestina.fluorita 1.0

// The room lit by whatever is on screen.
//
// A picture that does not fill the window leaves bands, and a black band is a
// hole. This puts the item's own artwork behind it, scaled to cover and blurred
// past recognition, so the bands carry the colour of what you are looking at
// instead of the absence of it.
//
// It is the same artwork the card was already showing, which is the whole
// reason it costs nothing: no second decode, no frame grab, no sampling of the
// video surface. For a track that artwork is the embedded cover, so music gets
// lit by its own sleeve rather than being the one kind of content left dark.
//
// Blurred beyond legibility on purpose. This is light, not content: a
// recognisable second copy of the picture behind the picture competes with it.
Item {
    id: ambient

    // The artwork to light the room with. Empty means there is nothing cached
    // for this item, and an unlit canvas is the honest answer — inventing a
    // colour would be inventing information about the file.
    required property string source

    readonly property bool lit: ambient.source.length > 0
        && plate.status === Image.Ready

    // The artwork's own shape. A freedesktop thumbnail preserves the source's
    // aspect, so for a video this is the film's shape — which is what lets the
    // surface size the picture instead of letting it paint its own black bands
    // over the light. Zero until something is loaded.
    readonly property real contentAspect: plate.implicitWidth > 0 && plate.implicitHeight > 0
        ? plate.implicitWidth / plate.implicitHeight
        : 0

    // Never interactive: it sits under everything and answers to nothing.
    z: -1

    Image {
        id: plate

        anchors.fill: parent
        source: ambient.source
        // Cover, so there is never a band inside the light itself.
        fillMode: Image.PreserveAspectCrop
        autoTransform: true
        asynchronous: true
        // The blur destroys detail anyway, so this reads the thumbnail at a
        // fraction of the surface and saves the memory a full-size copy of
        // every opened item would cost.
        sourceSize.width: Math.max(1, Math.round(ambient.width / 4))
        sourceSize.height: Math.max(1, Math.round(ambient.height / 4))
        // Hidden behind its own blurred copy; showing both would double the
        // brightness and put a sharp edge back on screen.
        visible: false
    }

    MultiEffect {
        anchors.fill: parent
        source: plate
        visible: ambient.lit
        blurEnabled: true
        blur: 1
        blurMax: CelestinaTheme.glassBlurMax
        blurMultiplier: CelestinaTheme.glassBlurMultiplier
        // Dimmed and pulled back from full saturation: at full strength the
        // light competes with the picture it is supposed to sit behind.
        brightness: CelestinaTheme.ambientBrightness
        saturation: CelestinaTheme.ambientSaturation

        // Fades in with the artwork rather than snapping on when the decode
        // lands, which would read as a flash.
        opacity: ambient.lit ? 1 : 0

        Behavior on opacity {
            NumberAnimation {
                duration: CelestinaTheme.reducedMotion
                    ? 0 : CelestinaTheme.motionSlow
                easing.type: CelestinaTheme.easeStandard
            }
        }
    }
}
