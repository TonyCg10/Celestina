import QtQuick
import QtQuick.Layouts
import org.celestina.magnetita 1.0

// How the mirror should look, as the few choices worth a control.
//
// scrcpy has well over a hundred flags. The ones here are the ones that change
// what mirroring at a desk feels like: how sharp, how smooth, how much of the
// link it eats, where the sound comes out, and whether the phone's own screen
// stays lit. Everything else is either already right or a niche reachable by
// running scrcpy directly.
//
// Each choice is a closed set, never a free number, because the daemon refuses
// anything outside its contract — so the surface offers exactly what can be
// stored. Nothing is painted optimistically: a press asks, and the next
// confirmed snapshot is what re-binds the control.
Item {
    id: root

    required property DevicesModel devices
    signal dismissRequested

    // scrcpy cannot be reconfigured mid-stream, so a change made while the
    // mirror is up applies the next time it opens. Say so rather than let the
    // author wonder why nothing moved.
    readonly property bool appliesLater: root.devices.mirrorActive

    implicitHeight: sheet.implicitHeight

    Column {
        id: sheet
        width: parent.width
        spacing: CelestinaTheme.spaceSm

        CelestinaSectionLabel { text: qsTr("Espejo") }

        MirrorChoiceRow {
            width: parent.width
            label: qsTr("Nitidez")
            options: ["modest", "balanced", "sharp", "native"]
            labels: ["1080", "1440", "1920", qsTr("Nativa")]
            current: root.devices.mirrorResolution
            onChosen: function(value) { root.devices.setMirrorOption("resolution", value) }
        }

        MirrorChoiceRow {
            width: parent.width
            label: qsTr("Fluidez")
            options: ["calm", "smooth", "fluid"]
            labels: ["30 fps", "60 fps", "120 fps"]
            current: root.devices.mirrorRate
            onChosen: function(value) { root.devices.setMirrorOption("rate", value) }
        }

        MirrorChoiceRow {
            width: parent.width
            label: qsTr("Calidad")
            options: ["thrifty", "everyday", "generous"]
            labels: ["4 Mb/s", "6 Mb/s", "16 Mb/s"]
            current: root.devices.mirrorQuality
            onChosen: function(value) { root.devices.setMirrorOption("quality", value) }
        }

        MirrorChoiceRow {
            width: parent.width
            label: qsTr("Sonido")
            options: ["phone", "desktop"]
            labels: [qsTr("En el móvil"), qsTr("En el PC")]
            current: root.devices.mirrorAudio
            onChosen: function(value) { root.devices.setMirrorOption("audio", value) }
        }

        PluginRow {
            width: parent.width
            label: qsTr("Apagar la pantalla del móvil")
            enabledFlag: root.devices.mirrorScreenOff
            onToggleRequested: root.devices.setMirrorOption(
                                   "screenOff",
                                   root.devices.mirrorScreenOff ? "false" : "true")
        }

        PluginRow {
            width: parent.width
            label: qsTr("Mantener el móvil despierto")
            enabledFlag: root.devices.mirrorStayAwake
            onToggleRequested: root.devices.setMirrorOption(
                                   "stayAwake",
                                   root.devices.mirrorStayAwake ? "false" : "true")
        }

        Text {
            width: parent.width
            visible: root.appliesLater
            text: qsTr("El espejo está abierto: los cambios se aplican la próxima vez.")
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            wrapMode: Text.WordWrap
        }
    }
}
