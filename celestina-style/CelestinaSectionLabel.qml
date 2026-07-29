import QtQuick

// The shared uppercase eyebrow used to name grouped regions. Position and
// available width belong to the consumer; type, tracking and ink do not.
Text {
    enum Size {
        Compact,
        Regular
    }

    property real textScale: 1.0
    property int size: CelestinaSectionLabel.Compact

    color: CelestinaTheme.textFaint
    font.family: CelestinaTheme.sansFamily
    font.pixelSize: Math.round((size === CelestinaSectionLabel.Regular
                                ? CelestinaTheme.fontRowSecondary
                                : CelestinaTheme.fontMini) * textScale)
    font.letterSpacing: CelestinaTheme.sectionLetterSpacing
    font.weight: CelestinaTheme.weightDemiBold
}
