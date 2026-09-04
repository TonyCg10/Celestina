pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.fluorita 1.0

// The tools, as icons.
//
// Icon-first with one hover circle for every action, and a capsule around each
// pair that belongs together — the two turns, the two mirrors — which is the
// suite's anatomy for actions that are one idea with two directions.
//
// It owns no truth: which tool is armed lives in the surface above it, and
// whether an action is possible comes from the editor. Nothing here decides
// what an edit is.
Item {
    id: bar

    required property FluoritaEditor editor
    // The armed tool: one of `none`, `crop`, `text`, `stroke`, `line`,
    // `arrow`, `rect`, `ellipse`, `highlight`, `redact`.
    required property string tool
    // The colour new marks are drawn in.
    required property color ink
    // How close the picture is being looked at, so the magnifier can say
    // whether pressing it moves in or back out.
    required property bool zoomed

    signal zoomToggled()

    // The palette. Six semantic tokens rather than a colour wheel: a mark on a
    // photograph has to be *seen*, and the ones that read over any picture are
    // few and already named by the style.
    readonly property var inkChoices: [
        CelestinaTheme.danger,
        CelestinaTheme.warning,
        CelestinaTheme.success,
        CelestinaTheme.accent,
        CelestinaTheme.text,
        CelestinaTheme.canvasInk
    ]

    signal toolPicked(string tool)
    signal inkPicked(color colour)
    signal turned(bool clockwise)
    signal mirrored(bool horizontal)
    signal saveRequested(bool replace)
    signal discardRequested()

    implicitHeight: row.implicitHeight + CelestinaTheme.spaceMd * 2
    implicitWidth: row.implicitWidth + CelestinaTheme.spaceLg * 2

    Accessible.role: Accessible.ToolBar
    Accessible.name: qsTr("Herramientas de edición")

    GlassSurface {
        anchors.fill: parent
        cornerRadius: CelestinaTheme.radiusPill
    }

    // The pill owns its box. Without this a click in a gap between two icons
    // fell through to the canvas and, with a tool armed, started a shape on the
    // picture panned underneath.
    CelestinaInputShield { }

    // The shield leaves the wheel to the content by contract; here the content
    // is the picture, and Ctrl+wheel over the toolbar zoomed it at the pointer.
    WheelHandler {
        acceptedModifiers: Qt.ControlModifier
        target: null
        onWheel: function(event) { event.accepted = true }
    }

    RowLayout {
        id: row

        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceMd

        // The canvas, as two pairs: turn and mirror.
        Row {
            spacing: 0

            CelestinaIconButton {
                iconName: "rotate-ccw"
                helpText: qsTr("Girar a la izquierda")
                enabled: !bar.editor.saving
                onClicked: bar.turned(false)
            }

            CelestinaIconButton {
                iconName: "rotate-ccw"
                helpText: qsTr("Girar a la derecha")
                enabled: !bar.editor.saving
                // The same glyph read the other way round: two icons for one
                // idea would be two things to learn.
                transform: Scale {
                    origin.x: CelestinaTheme.iconSm
                    xScale: -1
                }
                onClicked: bar.turned(true)
            }
        }

        Row {
            spacing: 0

            CelestinaIconButton {
                iconName: "symlink"
                helpText: qsTr("Voltear en horizontal")
                enabled: !bar.editor.saving
                onClicked: bar.mirrored(true)
            }

            CelestinaIconButton {
                iconName: "symlink"
                helpText: qsTr("Voltear en vertical")
                enabled: !bar.editor.saving
                rotation: 90
                onClicked: bar.mirrored(false)
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        Repeater {
            model: [
                { tool: "crop", icon: "scissors", label: qsTr("Recortar") },
                { tool: "text", icon: "type", label: qsTr("Texto") },
                { tool: "stroke", icon: "pencil", label: qsTr("Trazo") },
                { tool: "line", icon: "minus", label: qsTr("Línea") },
                { tool: "arrow", icon: "arrow-right", label: qsTr("Flecha") },
                { tool: "rect", icon: "layout-template", label: qsTr("Rectángulo") },
                { tool: "ellipse", icon: "circle-alert", label: qsTr("Elipse") },
                { tool: "highlight", icon: "paintbrush", label: qsTr("Resaltar") },
                { tool: "redact", icon: "eye-off", label: qsTr("Ocultar") }
            ]

            delegate: CelestinaIconButton {
                required property var modelData

                iconName: modelData.icon
                helpText: modelData.label
                enabled: !bar.editor.saving
                // A toggle: `checkable` paints Selected while it is armed. The
                // truth stays in the surface — the click re-evaluates `checked`
                // through `bar.tool`, never the other way round.
                role: CelestinaButton.Ghost
                checkable: true
                checked: bar.tool === modelData.tool
                // Arming the tool that is already armed disarms it, so leaving
                // a tool never means finding another one to pick.
                onClicked: bar.toolPicked(bar.tool === modelData.tool ? "none" : modelData.tool)
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        CelestinaIconButton {
            iconName: "search"
            helpText: bar.zoomed ? qsTr("Ver la imagen entera")
                                 : qsTr("Acercar")
            checkable: true
            checked: bar.zoomed
            onClicked: bar.zoomToggled()
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        // The colour, as the thing itself rather than as an icon of it.
        Row {
            spacing: CelestinaTheme.spaceXs

            Repeater {
                model: bar.inkChoices

                delegate: Item {
                    id: swatch

                    required property color modelData

                    readonly property bool current: Qt.colorEqual(bar.ink, swatch.modelData)

                    implicitWidth: CelestinaTheme.iconMd
                    implicitHeight: CelestinaTheme.iconMd
                    activeFocusOnTab: true

                    Accessible.role: Accessible.RadioButton
                    Accessible.name: qsTr("Color del trazo")
                    Accessible.checked: swatch.current
                    Accessible.onPressAction: bar.inkPicked(swatch.modelData)

                    // The same hover circle every icon action wears, so a
                    // swatch answers the pointer the way the glyphs beside it
                    // do instead of sitting inert until it is clicked.
                    Rectangle {
                        anchors.fill: parent
                        radius: CelestinaTheme.radiusPill
                        color: tap.pressed
                            ? CelestinaTheme.surfaceStrong
                            : hover.hovered
                              ? CelestinaTheme.surfaceHover
                              : CelestinaTheme.clear

                        Behavior on color {
                            ColorAnimation {
                                duration: CelestinaTheme.reducedMotion
                                    ? 0 : CelestinaTheme.motionFast
                            }
                        }
                    }

                    Rectangle {
                        anchors.centerIn: parent
                        width: swatch.current ? CelestinaTheme.iconSm
                                              : Math.round(CelestinaTheme.iconSm * 0.7)
                        height: width
                        radius: CelestinaTheme.radiusPill
                        color: swatch.modelData
                        border.width: CelestinaTheme.borderHairline
                        border.color: CelestinaTheme.dividerStrong
                        // Sinks under the finger by the suite's recoil, so the
                        // press is seen before the colour changes.
                        scale: tap.pressed ? CelestinaTheme.pressRecoilScale : 1
                        transformOrigin: Item.Center

                        Behavior on width {
                            NumberAnimation {
                                duration: CelestinaTheme.reducedMotion
                                    ? 0 : CelestinaTheme.motionFast
                                easing.type: CelestinaTheme.easeStandard
                            }
                        }

                        Behavior on scale {
                            NumberAnimation {
                                duration: CelestinaTheme.reducedMotion
                                    ? 0 : CelestinaTheme.motionFast
                                easing.type: CelestinaTheme.easeStandard
                            }
                        }
                    }

                    HoverHandler {
                        id: hover
                    }

                    CelestinaFocusRing {
                        target: swatch
                        cornerRadius: CelestinaTheme.radiusPill
                        visible: swatch.activeFocus
                    }

                    TapHandler {
                        id: tap

                        onTapped: bar.inkPicked(swatch.modelData)
                    }

                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_Space || event.key === Qt.Key_Return
                                || event.key === Qt.Key_Enter) {
                            bar.inkPicked(swatch.modelData)
                            event.accepted = true
                        }
                    }
                }
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        // Undo and redo: one idea with two directions, so one capsule.
        CelestinaCapsule {
            CelestinaIconButton {
                iconName: "undo"
                helpText: qsTr("Deshacer")
                role: CelestinaButton.Ghost
                enabled: bar.editor.canUndo && !bar.editor.saving
                onClicked: bar.editor.undo()
            }

            CelestinaIconButton {
                iconName: "redo"
                helpText: qsTr("Rehacer")
                role: CelestinaButton.Ghost
                enabled: bar.editor.canRedo && !bar.editor.saving
                onClicked: bar.editor.redo()
            }
        }

        Rectangle {
            Layout.preferredWidth: 1
            Layout.fillHeight: true
            Layout.topMargin: CelestinaTheme.spaceXs
            Layout.bottomMargin: CelestinaTheme.spaceXs
            color: CelestinaTheme.divider
        }

        // The two outcomes, and nothing between them.
        CelestinaIconButton {
            iconName: "copy"
            helpText: qsTr("Guardar una copia")
            enabled: bar.editor.edited && !bar.editor.saving
            role: CelestinaButton.Primary
            onClicked: bar.saveRequested(false)
        }

        CelestinaIconButton {
            iconName: "check"
            helpText: qsTr("Reemplazar el original")
            enabled: bar.editor.edited && !bar.editor.saving
            onClicked: bar.saveRequested(true)
        }

        CelestinaIconButton {
            iconName: "x"
            helpText: qsTr("Descartar los cambios")
            enabled: !bar.editor.saving
            onClicked: bar.discardRequested()
        }
    }
}
