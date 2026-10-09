import QtQuick

// The theme's two host-controlled accessibility inputs, bound once per window.
// An application's appearance adapter follows the suite's appearance file
// (`~/.config/celestina/appearance.toml`, `celestina-settings`) on a worker
// and queues its values; the window hands them here, and every token that
// reads `CelestinaTheme.reducedMotion` or `CelestinaTheme.textScale` follows.
// It takes values, never a controller, and draws nothing.
QtObject {
    id: appearance

    required property bool reducedMotion
    required property real textScale

    readonly property Binding motionBinding: Binding {
        target: CelestinaTheme
        property: "reducedMotion"
        value: appearance.reducedMotion
    }

    readonly property Binding scaleBinding: Binding {
        target: CelestinaTheme
        property: "textScale"
        value: appearance.textScale
    }
}
