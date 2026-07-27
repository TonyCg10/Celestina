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

    component Section: Column {
        property string heading: ""
        width: sheet.width
        spacing: CelestinaTheme.spaceMd
        Text {
            text: parent.heading
            color: CelestinaTheme.textMuted
            font.family: win.sans
            font.pixelSize: CelestinaTheme.fontMini
            font.letterSpacing: 1.4
            font.weight: CelestinaTheme.weightDemiBold
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
                            border.width: 1; border.color: CelestinaTheme.divider
                            Column {
                                anchors.centerIn: parent; spacing: 1
                                Text { text: "Aa 0123"; color: modelData.i; font.family: win.sans; font.pixelSize: 18; font.weight: 600; anchors.horizontalCenter: parent.horizontalCenter }
                                Text { text: modelData.n; color: modelData.i; font.family: win.sans; font.pixelSize: 10; anchors.horizontalCenter: parent.horizontalCenter }
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
                    Text { text: "display 34"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontDisplay; font.weight: 600 }
                    Text { text: "headerExpanded 30"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontHeaderExpanded; font.weight: 600 }
                    Text { text: "headerCollapsed 20 · title 17"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontHeaderCollapsed; font.weight: 600 }
                    Text { text: "rowTitle 15 medium — Documentos"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: CelestinaTheme.fontRowTitle; font.weight: 500 }
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
                    CelestinaButton { text: "Guardar"; primary: true }
                    CelestinaButton { text: "Vaciar papelera"; destructive: true }
                    CelestinaButton { text: "Desactivado"; enabled: false }
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
                        model: ["go-previous","go-next","go-up","go-home","view-refresh","view-sort-ascending","view-sort-descending","folder","file","symlink","user-trash","media-eject","phone","battery-charging","star","star-outline"]
                        Rectangle {
                            width: 64; height: 64; radius: CelestinaTheme.radiusSm; color: CelestinaTheme.card
                            Image {
                                anchors.centerIn: parent; width: 28; height: 28
                                source: Qt.resolvedUrl("../icons/" + modelData + ".svg")
                                sourceSize: Qt.size(28, 28); smooth: true
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
                            GradientStop { position: 0; color: "#387aff" }
                            GradientStop { position: 0.5; color: "#58db9c" }
                            GradientStop { position: 1; color: "#fc864c" }
                        }
                    }
                    GlassSurface {
                        anchors.centerIn: parent
                        width: 320; height: 130
                        backdropSource: glassBackdrop
                        liveCapture: true
                        elevation: 2
                        cornerRadius: CelestinaTheme.radiusLg
                        Text { anchors.centerIn: parent; text: "GlassSurface v2"; color: CelestinaTheme.text; font.family: win.sans; font.pixelSize: 18; font.weight: 600 }
                    }
                }
            }
        }
    }
}
