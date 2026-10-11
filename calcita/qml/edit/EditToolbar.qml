pragma ComponentBehavior: Bound
import QtQuick
import org.celestina.calcita 1.0

// The editing tools, a glass pill at the bottom of the window, in the
// grammar of Fluorita's editor: the four tools (a toggle each), the six
// theme colours, the size (a text box's S/M/L, a stroke's thin/medium/
// thick), undo and redo in one capsule, delete, the two save outcomes and
// leaving. It owns no truth: the window holds the armed tool, colour, size
// and selection, and the editor says what can be undone or saved.
//
// It is never cut: given less than every tool needs, the tools scroll
// sideways inside the pill (drag, wheel, or Tab to one out of sight).
Item {
    id: bar

    required property CalcitaEditor editor
    // `select`, `text`, `highlight` or `ink`.
    required property string tool
    // One of `CelestinaTheme.iconAccentKeys`.
    required property string colorKey
    // 0, 1 or 2.
    required property int sizeIndex
    // Something is selected, so delete applies to it.
    required property bool hasSelection

    signal toolPicked(string tool)
    signal colorPicked(string key)
    signal sizePicked(int index)
    signal deleteRequested()
    // `replace`: «Guardar solo la editada»; else «Guardar ambas».
    signal saveRequested(bool replace)
    signal doneRequested()

    readonly property bool busy: bar.editor.saving
    // The size names the armed tool gives its three steps.
    readonly property var sizeNames: bar.tool === "ink"
                                     ? [qsTr("Trazo fino"), qsTr("Trazo medio"), qsTr("Trazo grueso")]
                                     : [qsTr("Texto pequeño"), qsTr("Texto mediano"), qsTr("Texto grande")]
    // Given less than every tool needs: the tools scroll.
    readonly property bool cut: row.implicitWidth > strip.width + 0.5

    objectName: "editToolbar"
    implicitHeight: row.implicitHeight + CelestinaTheme.spaceXs * 2
    implicitWidth: row.implicitWidth + CelestinaTheme.spaceSm * 2
    Accessible.role: Accessible.ToolBar
    Accessible.name: qsTr("Herramientas de edición")

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Elevated
        radiusOverride: CelestinaTheme.radiusPill
        Accessible.ignored: true
    }

    // The pill owns its box: a click between two tools never reaches the
    // page under it.
    CelestinaInputShield {
        anchors.fill: parent
    }

    Flickable {
        id: strip
        anchors.fill: parent
        anchors.leftMargin: CelestinaTheme.spaceSm
        anchors.rightMargin: CelestinaTheme.spaceSm
        contentWidth: bar.cut ? row.implicitWidth : strip.width
        contentHeight: strip.height
        flickableDirection: Flickable.HorizontalFlick
        boundsBehavior: Flickable.StopAtBounds
        interactive: bar.cut
        clip: bar.cut

        Row {
            id: row
            x: bar.cut ? 0 : (strip.width - row.implicitWidth) / 2
            y: (strip.height - row.implicitHeight) / 2
            spacing: CelestinaTheme.spaceSm

            Repeater {
                model: [
                    { "tool": "select", "label": qsTr("Seleccionar") },
                    { "tool": "text", "label": qsTr("Texto") },
                    { "tool": "highlight", "label": qsTr("Resaltar") },
                    { "tool": "ink", "label": qsTr("Dibujar") }
                ]

                delegate: CelestinaButton {
                    required property var modelData
                    objectName: "tool_" + modelData.tool
                    y: (row.height - height) / 2
                    text: modelData.label
                    role: CelestinaButton.Ghost
                    checkable: true
                    checked: bar.tool === modelData.tool
                    enabled: !bar.busy
                    onClicked: bar.toolPicked(modelData.tool)
                }
            }

            Rectangle {
                width: CelestinaTheme.borderHairline
                height: CelestinaTheme.iconMd
                anchors.verticalCenter: parent.verticalCenter
                color: CelestinaTheme.divider
            }

            // The colour, as the thing itself.
            Row {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 0

                Repeater {
                    model: CelestinaTheme.iconAccentKeys

                    delegate: Item {
                        id: swatch
                        required property string modelData
                        readonly property bool current: bar.colorKey === swatch.modelData

                        objectName: "swatch_" + swatch.modelData
                        implicitWidth: CelestinaTheme.iconMd + CelestinaTheme.spaceXs
                        implicitHeight: CelestinaTheme.iconMd + CelestinaTheme.spaceXs
                        activeFocusOnTab: true
                        Accessible.role: Accessible.RadioButton
                        Accessible.name: CelestinaTheme.iconAccentLabel(swatch.modelData)
                        Accessible.checked: swatch.current
                        Accessible.onPressAction: bar.colorPicked(swatch.modelData)

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusPill
                            color: swatchHover.hovered ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: swatch.current ? CelestinaTheme.iconSm : CelestinaTheme.iconSm * 0.7
                            height: width
                            radius: CelestinaTheme.radiusPill
                            color: CelestinaTheme.iconAccentColor(swatch.modelData)
                            border.width: CelestinaTheme.borderHairline
                            border.color: CelestinaTheme.dividerStrong

                            Behavior on width {
                                enabled: !CelestinaTheme.reducedMotion
                                NumberAnimation {
                                    duration: CelestinaTheme.motionFast
                                    easing.type: CelestinaTheme.easeStandard
                                }
                            }
                        }

                        HoverHandler {
                            id: swatchHover
                        }

                        CelestinaFocusRing {
                            target: swatch
                            cornerRadius: CelestinaTheme.radiusPill
                            shown: swatch.activeFocus
                        }

                        TapHandler {
                            onTapped: bar.colorPicked(swatch.modelData)
                        }

                        Keys.onSpacePressed: bar.colorPicked(swatch.modelData)
                        Keys.onReturnPressed: bar.colorPicked(swatch.modelData)
                    }
                }
            }

            Rectangle {
                width: CelestinaTheme.borderHairline
                height: CelestinaTheme.iconMd
                anchors.verticalCenter: parent.verticalCenter
                color: CelestinaTheme.divider
            }

            // The size: three dots, each as big as what it makes.
            CelestinaCapsule {
                anchors.verticalCenter: parent.verticalCenter

                Repeater {
                    model: 3

                    delegate: Item {
                        id: step
                        required property int index
                        readonly property bool current: bar.sizeIndex === step.index

                        objectName: "size_" + step.index
                        implicitWidth: CelestinaTheme.iconMd + CelestinaTheme.spaceXs
                        implicitHeight: CelestinaTheme.iconMd + CelestinaTheme.spaceXs
                        activeFocusOnTab: true
                        Accessible.role: Accessible.RadioButton
                        Accessible.name: bar.sizeNames[step.index]
                        Accessible.checked: step.current
                        Accessible.onPressAction: bar.sizePicked(step.index)

                        Rectangle {
                            anchors.fill: parent
                            radius: CelestinaTheme.radiusPill
                            color: step.current ? CelestinaTheme.surfaceSelected
                                                : stepHover.hovered ? CelestinaTheme.surfaceHover
                                                                    : CelestinaTheme.clear
                        }

                        Rectangle {
                            anchors.centerIn: parent
                            width: CelestinaTheme.spaceXs * (step.index + 1)
                            height: width
                            radius: CelestinaTheme.radiusPill
                            color: CelestinaTheme.text
                        }

                        HoverHandler {
                            id: stepHover
                        }

                        CelestinaFocusRing {
                            target: step
                            cornerRadius: CelestinaTheme.radiusPill
                            shown: step.activeFocus
                        }

                        TapHandler {
                            onTapped: bar.sizePicked(step.index)
                        }

                        Keys.onSpacePressed: bar.sizePicked(step.index)
                        Keys.onReturnPressed: bar.sizePicked(step.index)
                    }
                }
            }

            Rectangle {
                width: CelestinaTheme.borderHairline
                height: CelestinaTheme.iconMd
                anchors.verticalCenter: parent.verticalCenter
                color: CelestinaTheme.divider
            }

            // Undo and redo: one idea with two directions, one capsule.
            CelestinaCapsule {
                anchors.verticalCenter: parent.verticalCenter

                CelestinaIconButton {
                    objectName: "undoButton"
                    iconName: "undo"
                    helpText: qsTr("Deshacer")
                    role: CelestinaButton.Ghost
                    enabled: bar.editor.canUndo && !bar.busy
                    onClicked: bar.editor.undo()
                }

                CelestinaIconButton {
                    objectName: "redoButton"
                    iconName: "redo"
                    helpText: qsTr("Rehacer")
                    role: CelestinaButton.Ghost
                    enabled: bar.editor.canRedo && !bar.busy
                    onClicked: bar.editor.redo()
                }
            }

            CelestinaIconButton {
                objectName: "deleteButton"
                anchors.verticalCenter: parent.verticalCenter
                iconName: "user-trash"
                helpText: qsTr("Eliminar")
                role: CelestinaButton.Ghost
                enabled: bar.hasSelection && !bar.busy
                onClicked: bar.deleteRequested()
            }

            Rectangle {
                width: CelestinaTheme.borderHairline
                height: CelestinaTheme.iconMd
                anchors.verticalCenter: parent.verticalCenter
                color: CelestinaTheme.divider
            }

            // The two outcomes, by the names the author gave them.
            CelestinaIconButton {
                objectName: "saveBothButton"
                anchors.verticalCenter: parent.verticalCenter
                iconName: "copy"
                helpText: qsTr("Guardar ambas")
                role: CelestinaButton.Primary
                enabled: bar.editor.edited && !bar.busy
                onClicked: bar.saveRequested(false)
            }

            CelestinaIconButton {
                objectName: "saveEditedButton"
                anchors.verticalCenter: parent.verticalCenter
                iconName: "check"
                helpText: qsTr("Guardar solo la editada")
                role: CelestinaButton.Ghost
                enabled: bar.editor.edited && !bar.busy
                onClicked: bar.saveRequested(true)
            }

            // Leaving: the window asks first when there are changes.
            CelestinaIconButton {
                objectName: "doneButton"
                anchors.verticalCenter: parent.verticalCenter
                iconName: "x"
                helpText: qsTr("Terminar de editar")
                role: CelestinaButton.Ghost
                enabled: !bar.busy
                onClicked: bar.doneRequested()
            }
        }
    }
}
