// The on-screen display: what a device is at, in the corner, for a moment.
//
// It shows readings, never requests. The host hands it a value a provider
// actually published, so a key that changed nothing raises nothing here, and a
// bar is drawn only when there is a level to draw — a device that reported no
// level says so in words instead of showing an empty bar at zero.
//
// Nothing in this file talks to a provider or a controller: it is handed four
// plain properties and paints them. It never takes focus or the keyboard
// either, which is a property of its surface (see `OverlaySurface`'s
// `Notification` placement), so no control here is interactive.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import QtQuick.Window

Window {
    id: osd

    // `volume`, `microphone` or `brightness`. The host owns the vocabulary;
    // an unknown kind is shown by name rather than dropped, because a display
    // that silently painted nothing would look like a broken key.
    required property string kind
    // Whole percent, or negative when the provider reported no level.
    required property int percent
    required property bool muted
    // Which device this is about, when the session has more than one of them.
    required property string label
    required property bool reducedMotion

    readonly property bool hasLevel: percent >= 0
    readonly property string headline: {
        if (osd.kind === "volume")
            return qsTr("Volumen");
        if (osd.kind === "microphone")
            return qsTr("Micrófono");
        if (osd.kind === "brightness")
            return osd.label.length > 0 ? qsTr("Brillo — %1").arg(osd.label)
                                        : qsTr("Brillo");
        return osd.kind;
    }
    readonly property string valueText: {
        if (osd.muted)
            return qsTr("Silenciado");
        if (!osd.hasLevel)
            return qsTr("Sin lectura");
        return qsTr("%1 %").arg(osd.percent);
    }
    // What a screen reader is told, in one sentence: the same two facts the
    // eye gets from the title and the number.
    readonly property string spokenText: qsTr("%1: %2").arg(osd.headline).arg(osd.valueText)

    width: 260
    height: 96
    color: CelestinaTheme.clear
    title: qsTr("Indicador en pantalla de Celestina")

    Component.onCompleted: CelestinaTheme.reducedMotion = osd.reducedMotion

    Item {
        id: scene

        anchors.fill: parent

        GlassCard {
            id: card

            anchors.fill: parent
            backdropSource: scene
            // It reports state and cannot be acted on, so it is neither a
            // dialog nor a button to assistive technology.
            Accessible.role: Accessible.StaticText
            Accessible.name: osd.spokenText
            // The value changes while the card stays up, so the announcement
            // has to follow it rather than being read once at creation.
            Accessible.description: osd.spokenText

            Column {
                anchors.fill: parent
                anchors.margins: CelestinaTheme.spaceLg
                spacing: CelestinaTheme.spaceSm

                Row {
                    width: parent.width
                    spacing: CelestinaTheme.spaceSm

                    Text {
                        id: titleText

                        width: parent.width - valueLabel.implicitWidth - parent.spacing
                        text: osd.headline
                        color: CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontBody
                        font.weight: CelestinaTheme.weightDemiBold
                        elide: Text.ElideRight
                    }

                    Text {
                        id: valueLabel

                        anchors.verticalCenter: titleText.verticalCenter
                        text: osd.valueText
                        // A silenced device keeps the level it remembers, and
                        // the reading says it is not being heard rather than
                        // pretending it moved.
                        color: osd.muted ? CelestinaTheme.textMuted : CelestinaTheme.text
                        font.family: CelestinaTheme.sansFamily
                        font.features: CelestinaTheme.fontFeaturesTabular
                        font.pixelSize: CelestinaTheme.fontBody
                    }
                }

                // A meter, not a control: there is nothing to drag here, and a
                // slider would offer an interaction this surface cannot accept
                // because it never takes a pointer or the keyboard.
                Rectangle {
                    id: track

                    width: parent.width
                    height: CelestinaTheme.spaceXs
                    radius: CelestinaTheme.radiusPill
                    visible: osd.hasLevel
                    // The same track the system slider draws, so a level reads
                    // the same whether it is being shown or being set.
                    color: CelestinaTheme.divider

                    Rectangle {
                        id: fill

                        height: parent.height
                        radius: parent.radius
                        width: parent.width * Math.max(0, Math.min(1, osd.percent / 100))
                        color: osd.muted ? CelestinaTheme.textMuted : CelestinaTheme.accent

                        // The level moving is the whole point of the display,
                        // so reduced motion keeps the value and drops the
                        // travel rather than the other way round.
                        Behavior on width {
                            enabled: !CelestinaTheme.reducedMotion
                            NumberAnimation {
                                duration: CelestinaTheme.motionFast
                                easing.type: CelestinaTheme.easeStandard
                            }
                        }
                    }
                }

                Text {
                    width: parent.width
                    visible: !osd.hasLevel && !osd.muted
                    text: qsTr("El proveedor no informó de ningún nivel para este dispositivo.")
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                    wrapMode: Text.WordWrap
                }
            }
        }
    }
}
