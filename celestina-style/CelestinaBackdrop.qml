import QtQuick

// Canonical L0 window backdrop. Apps may place restrained decorative children
// inside it, but the base gradient and canvas colour stay suite-owned.
Rectangle {
    color: CelestinaTheme.canvas

    gradient: Gradient {
        orientation: Gradient.Horizontal
        GradientStop { position: 0; color: CelestinaTheme.gradientStart }
        GradientStop { position: 0.55; color: CelestinaTheme.gradientMid }
        GradientStop { position: 1; color: CelestinaTheme.gradientEnd }
    }
}
