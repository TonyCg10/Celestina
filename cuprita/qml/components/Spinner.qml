import QtQuick
import org.celestina.cuprita 1.0

// The suite's refresh glyph turning while something is being read or done.
// It stands still when reduced motion is asked for.
CelestinaIcon {
    id: spinner

    width: CelestinaTheme.iconMd
    height: CelestinaTheme.iconMd
    name: "view-refresh"
    Accessible.ignored: true

    // The glyph is symmetric under a half turn: one half turn per cycle
    // reads as continuous spinning.
    RotationAnimator on rotation {
        from: 0
        to: 180
        duration: CelestinaTheme.motionCeiling
        loops: Animation.Infinite
        running: spinner.visible && !CelestinaTheme.reducedMotion
    }
}
