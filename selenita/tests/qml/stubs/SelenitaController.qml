pragma Singleton
import QtQuick

// Stands in for the Rust `SelenitaController` singleton under qmltestrunner,
// where the binary's types do not exist: the same properties, at rest, with
// the fakes on as `SELENITA_FAKE=1` sets them.
QtObject {
    property bool appearanceReducedMotion: false
    property real appearanceTextScale: 1.0
    readonly property bool smokeReport: false
    readonly property bool fake: true
}
