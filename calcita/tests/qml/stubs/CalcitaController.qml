pragma Singleton
import QtQuick

// Stands in for the Rust `CalcitaController` singleton under qmltestrunner,
// where the binary's types do not exist: the same properties, at rest.
QtObject {
    property bool appearanceReducedMotion: false
    property real appearanceTextScale: 1.0
    readonly property bool smokeReport: false
}
