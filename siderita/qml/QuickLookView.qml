import QtQuick
import QtQuick.Controls
import QtQuick.Controls.impl
import QtQuick.Layouts
import org.celestina.siderita 1.0

    // ── Quick-look preview (spacebar) ────────────────────────────────
    // A read-only peek at the selected entry without opening an app:
    // images render full-size, text/code shows in a monospace pane, and
    // anything else (folders, video, audio, binaries) gets an info card.
    // ↑/↓ browse the folder live; Space / Esc / click-outside dismiss.
Rectangle {
    id: quickLookView
    property var controller
    property var owner
    property var panel   // mainPanel: ayudas de selección y medios
    anchors.fill: parent
    z: 70
    readonly property bool shown: owner.quickLookOpen
    // Fades rather than pops. Opacity only: a scale transform on a
    // glass surface desyncs its backdrop sampling (see a995619), so the
    // motion here never touches geometry.
    visible: opacity > 0.01
    opacity: shown ? 1 : 0
    Behavior on opacity {
        NumberAnimation {
            duration: CelestinaTheme.motionFast
            easing.type: CelestinaTheme.easeStandard
        }
    }
    color: Qt.rgba(0, 0, 0, 0.55)
    focus: quickLookView.shown

    // Everything is derived from the current selection, so stepping the
    // selection (below) re-previews with no extra state to keep in sync.
    readonly property int qlIndex: controller.indexForToken(controller.selectedToken)
    readonly property string qlName: qlIndex >= 0 && qlIndex < controller.entryNames.length
                                     ? controller.entryNames[qlIndex] : ""
    readonly property string qlPath: qlIndex >= 0 ? controller.entryPath(qlIndex) : ""
    readonly property string qlKind: qlIndex >= 0 ? controller.entryKind(qlIndex) : ""
    readonly property string qlMedia: panel.mediaKind(qlName)
    readonly property bool qlIsImage: qlMedia === "image"
    // Read text lazily and only when it could be text — the controller
    // returns "" for a directory, an image or a binary, which the body
    // reads as "show the info card instead".
    readonly property string qlText: (owner.quickLookOpen && qlKind !== "directory"
                                      && !qlIsImage && qlPath.length > 0)
                                     ? controller.previewText(qlPath) : ""
    readonly property bool qlHasText: qlText.length > 0

    // Per-segment encode so spaces / #, ? etc. in a name survive the
    // file:// URL without mangling the path separators.
    function fileUrl(p) {
        return "file://" + p.split("/").map(encodeURIComponent).join("/")
    }

    MouseArea {
        anchors.fill: parent
        onClicked: owner.quickLookOpen = false
    }
    Keys.onPressed: function(event) {
        if (event.key === Qt.Key_Escape || event.key === Qt.Key_Space) {
            owner.quickLookOpen = false
            event.accepted = true
        } else if (event.key === Qt.Key_Down || event.key === Qt.Key_Right) {
            owner.quickLookStep(1)
            event.accepted = true
        } else if (event.key === Qt.Key_Up || event.key === Qt.Key_Left) {
            owner.quickLookStep(-1)
            event.accepted = true
        } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            owner.quickLookOpen = false
            controller.activateToken(controller.selectedToken)
            event.accepted = true
        }
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(720, owner.width - 64)
        height: Math.min(owner.height - 80, 640)
        backdropSource: mainPanel
        // (not transform-scaled — a scale transform desynced the glass backdrop)
        Accessible.role: Accessible.Dialog
        Accessible.name: "Vista previa"

        MouseArea { anchors.fill: parent }   // clicks on the card don't dismiss

        IconImage {
            id: qlIcon
            x: 18
            y: 16
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            name: panel.mediaIconName(quickLookView.qlKind, quickLookView.qlMedia, quickLookView.qlPath)
            source: CelestinaTheme.fallbackIcon(
                        quickLookView.qlKind === "directory" ? "folder" : "file")
            color: quickLookView.qlKind === "directory" ? CelestinaTheme.accent
                                                        : CelestinaTheme.textMuted
        }
        Text {
            anchors.left: qlIcon.right
            anchors.leftMargin: 10
            anchors.right: parent.right
            anchors.rightMargin: 18
            y: 17
            text: quickLookView.qlName
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            font.weight: CelestinaTheme.weightDemiBold
            elide: Text.ElideMiddle
        }

        Item {
            id: qlBody
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.top: qlIcon.bottom
            anchors.topMargin: 12
            anchors.bottom: qlHint.top
            anchors.bottomMargin: 8
            anchors.leftMargin: 16
            anchors.rightMargin: 16
            clip: true

            // (1) Image — the real file, capped so a huge photo can't
            // blow up memory; not cached (previews are transient).
            Image {
                anchors.fill: parent
                visible: quickLookView.qlIsImage
                source: quickLookView.qlIsImage
                        ? quickLookView.fileUrl(quickLookView.qlPath) : ""
                sourceSize.width: 1920
                sourceSize.height: 1920
                fillMode: Image.PreserveAspectFit
                asynchronous: true
                cache: false
                smooth: true
                mipmap: true
            }

            // (2) Text / code
            ScrollView {
                anchors.fill: parent
                visible: !quickLookView.qlIsImage && quickLookView.qlHasText
                clip: true
                TextArea {
                    readOnly: true
                    text: quickLookView.qlText
                    wrapMode: TextArea.NoWrap
                    selectByMouse: true
                    background: null
                    color: CelestinaTheme.text
                    font.family: CelestinaTheme.monoFamily
                    font.pixelSize: CelestinaTheme.fontCaption
                }
            }

            // (3) No renderable preview — a centred glyph + reason.
            Column {
                anchors.centerIn: parent
                spacing: 12
                visible: !quickLookView.qlIsImage && !quickLookView.qlHasText
                IconImage {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 56
                    height: 56
                    name: panel.mediaIconName(quickLookView.qlKind,
                                                  quickLookView.qlMedia, quickLookView.qlPath)
                    sourceSize: Qt.size(width, height)
                    source: CelestinaTheme.fallbackIcon(
                                quickLookView.qlKind === "directory" ? "folder" : "file")
                    color: quickLookView.qlKind === "directory"
                           ? CelestinaTheme.accent : CelestinaTheme.textMuted
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: quickLookView.qlKind === "directory" ? "Carpeta"
                        : quickLookView.qlMedia === "video"
                          ? "Vídeo — vista previa en Fluorita (próximamente)"
                        : quickLookView.qlMedia === "audio"
                          ? "Audio — vista previa en Fluorita (próximamente)"
                        : "Sin vista previa"
                    color: CelestinaTheme.textMuted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                }
            }
        }

        Text {
            id: qlHint
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            anchors.margins: 14
            horizontalAlignment: Text.AlignHCenter
            text: "Espacio o Esc para cerrar   ·   ↑ ↓ para navegar"
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            opacity: 0.8
        }
    }
}
