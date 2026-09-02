import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// El volumen de Fluorita sobre el control compartido — mismo patrón que
// `SeekBar`: la anatomía vive en `CelestinaSlider`, aquí quedan sólo las
// palabras de este valor.
RowLayout {
    id: bar

    required property real level

    signal volumeRequested(real level)

    // El icono recuerda a qué volumen volver al desmutear. Sólo se guarda
    // mientras suena algo: guardar un 0 dejaría el botón sin adónde volver.
    property real rememberedLevel: 1.0
    readonly property bool muted: bar.level <= 0

    onLevelChanged: if (bar.level > 0) bar.rememberedLevel = bar.level

    spacing: CelestinaTheme.spaceSm

    CelestinaIconButton {
        iconName: bar.muted ? "media-volume-muted" : "media-volume"
        helpText: bar.muted ? qsTr("Activar sonido") : qsTr("Silenciar")
        onClicked: bar.muted
            ? bar.volumeRequested(bar.rememberedLevel > 0 ? bar.rememberedLevel : 1.0)
            : bar.volumeRequested(0)
    }

    CelestinaSlider {
        id: slider

        Layout.preferredWidth: 96
        // A control's height rather than the track's own: the track centres
        // in it and the pointer hits it.
        Layout.preferredHeight: CelestinaTheme.controlHeightXs
        wheelEnabled: true
        value: bar.level
        to: 1
        // Un paso que se nota sin perder matices: veinte pasos de un extremo
        // al otro.
        step: 0.05

        Accessible.name: qsTr("Volumen")
        Accessible.description: qsTr("Flechas para subir o bajar el volumen")

        onMoved: function(target) { bar.volumeRequested(target) }
    }
}
