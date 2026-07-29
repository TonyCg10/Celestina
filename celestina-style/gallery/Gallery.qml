import QtQuick
import QtQuick.Controls
import CelestinaStyle

// ─── Gallery ──────────────────────────────────────────────────────────────────
// The style's living review surface (DESIGN §7): every token, control and glass
// surface on one scrollable screen, so a change is seen whole and a regression
// shows here before it reaches an app. Dev-only — run it with the plain QML
// runtime, never shipped:
//
//   celestina-style/gallery/run.sh          (offscreen PNG or a real window)
//
// Glass and shadow need a real GPU session; the rest renders offscreen.
// ──────────────────────────────────────────────────────────────────────────────
Window {
    id: win
    width: 940
    height: 1180
    visible: true
    color: CelestinaTheme.canvas
    title: "CelestinaStyle — Gallery"

    // The gallery loads Inter by file path; a compiled app gets it from its qrc.
    FontLoader {
        id: inter
        source: Qt.resolvedUrl("../fonts/InterVariable.ttf")
    }
    readonly property string sans: inter.status === FontLoader.Ready ? inter.name : CelestinaTheme.sansFamily

    CelestinaBackdrop {
        anchors.fill: parent
    }

    component Section: Column {
        property string heading: ""
        width: sheet.width
        spacing: CelestinaTheme.spaceMd
        CelestinaSectionLabel {
            text: parent.heading
            font.family: win.sans
        }
    }

    Flickable {
        id: flick
        anchors.fill: parent
        contentWidth: width
        contentHeight: sheet.implicitHeight + 80
        clip: true

        Column {
            id: sheet
            x: 32
            y: 32
            width: win.width - 64
            spacing: 34

            Text {
                text: "CelestinaStyle"
                color: CelestinaTheme.text
                font.family: win.sans
                font.pixelSize: CelestinaTheme.fontDisplay
                font.weight: CelestinaTheme.weightDemiBold
            }

            // ── Surface → ink pairs ────────────────────────────────────────
            Section {
                heading: "SURFACE → INK"
                Grid {
                    columns: 4
                    spacing: 12
                    Repeater {
                        model: [
                            { s: CelestinaTheme.canvas,     i: CelestinaTheme.canvasInk,     n: "canvas" },
                            { s: CelestinaTheme.card,       i: CelestinaTheme.cardInk,       n: "card" },
                            { s: CelestinaTheme.elevated,   i: CelestinaTheme.elevatedInk,   n: "elevated" },
                            { s: CelestinaTheme.accent,     i: CelestinaTheme.accentInk,     n: "accent" },
                            { s: CelestinaTheme.danger,     i: CelestinaTheme.dangerInk,     n: "danger" },
                            { s: CelestinaTheme.success,    i: CelestinaTheme.successInk,    n: "success" },
                            { s: CelestinaTheme.warning,    i: CelestinaTheme.warningInk,    n: "warning" },
                            { s: CelestinaTheme.dangerFill, i: CelestinaTheme.dangerFillInk, n: "dangerFill" }
                        ]
                        Rectangle {
                            width: 200; height: 66
                            radius: CelestinaTheme.radiusSm
                            color: modelData.s
                            border.width: CelestinaTheme.borderHairline; border.color: CelestinaTheme.divider
                            Column {
                                anchors.centerIn: parent; spacing: 1
                                Text { text: "Aa 0123"; color: modelData.i; font.family: win.sans; font.pixelSize: CelestinaTheme.iconSm; font.weight: CelestinaTheme.weightDemiBold; anchors.horizontalCenter: parent.horizontalCenter }
                                Text { text: modelData.n; color: modelData.i; font.family: win.sans; font.pixelSize: CelestinaTheme.fontMini; anchors.horizontalCenter: parent.horizontalCenter }
                            }
                        }
                    }
                }
            }

            Section {
                heading: "SEMANTIC SURFACES"
                Row {
                    spacing: 12
                    Repeater {
                        model: [
                            { role: CelestinaSurface.Panel, name: "Panel" },
                            { role: CelestinaSurface.Grouped, name: "Grouped" },
                            { role: CelestinaSurface.Content, name: "Content" },
                            { role: CelestinaSurface.Tonal, name: "Tonal" },
                            { role: CelestinaSurface.Selected, name: "Selected" }
                        ]
                        CelestinaSurface {
                            id: surfaceSpec
                            width: 158
                            height: 72
                            role: modelData.role
                            Text {
                                anchors.centerIn: parent
                                text: modelData.name
                                color: surfaceSpec.ink
                                font.family: win.sans
                                font.pixelSize: CelestinaTheme.fontBody
                            }
                        }
                    }
                }
            }

            // ── Typography ─────────────────────────────────────────────────
            Section {
                heading: "TYPE ROLES"
                Column {
                    spacing: 6
                    Text { text: "display 34"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontDisplay; font.weight: CelestinaTheme.weightDemiBold }
                    Text { text: "headerExpanded 30"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontHeaderExpanded; font.weight: CelestinaTheme.weightDemiBold }
                    Text { text: "headerCollapsed 20 · title 17"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontHeaderCollapsed; font.weight: CelestinaTheme.weightDemiBold }
                    Text { text: "rowTitle 15 medium — Documentos"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontRowTitle; font.weight: CelestinaTheme.weightMedium }
                    Text { text: "body 13 — the quick brown fox jumps over the lazy dog"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontBody }
                    Text { text: "rowSecondary 12 · caption 11 · mini 10 — 0123456789"; color: CelestinaTheme.textMuted; font.family: win.sans; font.pixelSize: CelestinaTheme.fontRowSecondary; font.features: CelestinaTheme.fontFeaturesTabular }
                }
            }

            // ── Buttons ────────────────────────────────────────────────────
            Section {
                heading: "BUTTONS"
                Row {
                    spacing: 14
                    CelestinaButton { text: "Cancelar" }
                    CelestinaButton { text: "Guardar"; role: CelestinaButton.Primary }
                    CelestinaButton { text: "Vaciar papelera"; role: CelestinaButton.Destructive }
                    CelestinaButton { text: "Sólo texto"; role: CelestinaButton.Ghost }
                    CelestinaButton { text: "Desactivado"; enabled: false }
                    CelestinaIconButton {
                        iconName: "settings"
                        fallbackIcon: "settings"
                        helpText: "Ajustes"
                    }
                    CelestinaIconButton {
                        density: CelestinaButton.Regular
                        role: CelestinaButton.Primary
                        iconName: ""
                        fallbackIcon: "media-play"
                        helpText: "Icono cuadrado"
                    }
                    CelestinaButton {
                        text: "Abrir modal"
                        onClicked: modalSpecimen.shown = true
                    }
                }
            }

            // ── Text field ─────────────────────────────────────────────────
            Section {
                heading: "TEXT FIELD"
                Row {
                    spacing: 14
                    CelestinaTextField { width: 220; placeholderText: "Nombre…" }
                    CelestinaTextField { width: 220; text: "documento.txt" }
                }
            }

            // ── Switches ───────────────────────────────────────────────────
            Section {
                heading: "SWITCH"
                Row {
                    spacing: 24
                    CelestinaSwitch { checked: true }
                    CelestinaSwitch { checked: false }
                    CelestinaSwitch { checked: true; enabled: false }
                }
            }

            // ── ListSection ────────────────────────────────────────────────
            ListSection {
                width: sheet.width
                title: "LIST SECTION — THE SIGNATURE"
                Repeater {
                    model: [["Portapapeles", true], ["Notificaciones", true], ["Compartir batería", false]]
                    Item {
                        required property var modelData
                        width: parent.width; height: 48
                        Text { anchors.verticalCenter: parent.verticalCenter; x: 16; text: modelData[0]; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontRowTitle }
                        CelestinaSwitch { anchors.verticalCenter: parent.verticalCenter; anchors.right: parent.right; anchors.rightMargin: 14; checked: modelData[1] }
                    }
                }
            }

            // ── Icons ──────────────────────────────────────────────────────
            Section {
                heading: "ICONS (Lucide)"
                Grid {
                    columns: 8
                    spacing: 10
                    Repeater {
                        model: ["go-previous","go-next","go-up","go-home","view-refresh","view-sort-ascending","view-sort-descending","folder","file","symlink","user-trash","media-eject","phone","battery-charging","star","star-outline","settings","music","key","media-skip-back","media-play","media-pause","media-skip-forward"]
                        Rectangle {
                            width: 64; height: 64; radius: CelestinaTheme.radiusSm; color: CelestinaTheme.card
                            CelestinaIcon {
                                anchors.centerIn: parent; width: 28; height: 28
                                name: modelData
                                fallbackName: "file"
                                tone: CelestinaIcon.Primary
                            }
                        }
                    }
                }
            }

            // ── Glass (needs a real GPU session) ───────────────────────────
            Section {
                heading: "GLASS — L2 ELEVATION (real session only)"
                Item {
                    width: sheet.width; height: 180
                    // A colourful backdrop so the blur + desaturation read.
                    Rectangle {
                        id: glassBackdrop
                        anchors.fill: parent
                        radius: CelestinaTheme.radiusLg
                        gradient: Gradient {
                            orientation: Gradient.Horizontal
                            GradientStop { position: 0; color: CelestinaTheme.accent }
                            GradientStop { position: 0.5; color: CelestinaTheme.success }
                            GradientStop { position: 1; color: CelestinaTheme.warning }
                        }
                    }
                    Row {
                        anchors.centerIn: parent
                        spacing: 24

                        GlassSurface {
                            width: 300; height: 130
                            backdropSource: glassBackdrop
                            liveCapture: true
                            elevation: 2
                            cornerRadius: CelestinaTheme.radiusLg
                            Text { anchors.centerIn: parent; text: "Regular"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.iconSm; font.weight: CelestinaTheme.weightDemiBold }
                        }

                        GlassSurface {
                            width: 300; height: 130
                            backdropSource: glassBackdrop
                            liveCapture: true
                            density: GlassSurface.Strong
                            cornerRadius: CelestinaTheme.radiusLg
                            Text { anchors.centerIn: parent; text: "Strong"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.iconSm; font.weight: CelestinaTheme.weightDemiBold }
                        }
                    }
                }
            }
        }
    }

    CelestinaModalLayer {
        id: modalSpecimen
        anchors.fill: parent
        z: 100
        onDismissRequested: shown = false

        GlassCard {
            anchors.centerIn: parent
            width: 360
            height: 180
            backdropSource: flick

            MouseArea { anchors.fill: parent }

            Text {
                anchors.horizontalCenter: parent.horizontalCenter
                y: 38
                text: "Capa modal compartida"
                color: CelestinaTheme.text
                font.family: win.sans
                font.pixelSize: CelestinaTheme.fontTitle
                font.weight: CelestinaTheme.weightDemiBold
            }

            CelestinaButton {
                anchors.horizontalCenter: parent.horizontalCenter
                anchors.bottom: parent.bottom
                anchors.bottomMargin: 30
                text: "Cerrar"
                role: CelestinaButton.Primary
                onClicked: modalSpecimen.shown = false
            }
        }
    }
}
