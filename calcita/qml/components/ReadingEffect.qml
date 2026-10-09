import QtQuick

// The dark reading mode's layer effect over the page view: the pages
// inverted and their hue turned half a circle (`shaders/reading.frag`,
// compiled to `.qsb` by `build.rs`). Images are inverted too; this is a way
// of reading, not a way of rendering. `amount` runs from 0 (the page as
// drawn) to 1.
ShaderEffect {
    // The page view's layer, given by `layer.effect`.
    property var source
    property real amount: 1

    fragmentShader: "qrc:/calcita/shaders/reading.frag.qsb"
}
