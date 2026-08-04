import QtQuick
import QtQuick.Window
import org.celestina.siderita 1.0

    // ── Quick-look preview (spacebar) ────────────────────────────────
    // A read-only peek at the selected entry without opening an app:
    // images render full-size, text/code shows in a monospace pane, and
    // anything else (folders, video, audio, binaries) gets an info card.
    // ↑/↓ browse the folder live; Space / Esc / click-outside dismiss.
CelestinaModalLayer {
    id: quickLookView
    property var controller
    property var owner
    property var panel   // mainPanel: ayudas de selección y medios
    property var player  // SideritaPlayer: el reproductor incrustado
    // GrafitaPreferences: the text size the reader chose, shared with Grafita's
    // own window through the file it stores. Read only here — a peek offers no
    // way to change it, and its own key map already owns Space and the arrows.
    property var reading
    anchors.fill: parent
    z: 70
    shown: owner.quickLookOpen
    dismissOnEscape: false
    onDismissRequested: owner.quickLookOpen = false

    // Everything is derived from the current selection, so stepping the
    // selection (below) re-previews with no extra state to keep in sync.
    readonly property int qlIndex: controller.indexForToken(controller.selectedToken)
    readonly property string qlName: qlIndex >= 0 && qlIndex < controller.entryNames.length
                                     ? controller.entryNames[qlIndex] : ""
    readonly property string qlPath: qlIndex >= 0 ? controller.entryPath(qlIndex) : ""
    readonly property string qlKind: qlIndex >= 0 ? controller.entryKind(qlIndex) : ""
    readonly property string qlMedia: panel.mediaKind(qlName)
    readonly property bool qlIsImage: qlMedia === "image"
    // El audio se reproduce aquí mismo; el vídeo todavía no tiene superficie en
    // este modal, así que se dice y se ofrece Intro para abrir Fluorita.
    readonly property bool qlIsAudio: qlMedia === "audio"
    readonly property bool qlIsVideo: qlMedia === "video"
    readonly property bool qlIsPlayable: qlIsAudio || qlIsVideo
    // Read text lazily and only when it could be text — the controller
    // returns "" for a directory, an image or a binary, which the body
    // reads as "show the info card instead".
    //
    // This is a renderer, not a decision. Whether an entry is *editable* text
    // is settled before this view ever opens, by Grafita's content probe on its
    // worker; anything that lands here was already refused as editable and is
    // shown read-only.
    // Una sesión a la vez: cada paso de selección cierra la anterior antes de
    // abrir nada, y cerrar el modal no deja ningún decodificador vivo.
    onQlPathChanged: quickLookView.syncPlayer()
    onShownChanged: {
        quickLookView.syncPlayer()
        if (quickLookView.shown)
            quickLookView.reading.reload()
    }

    function syncPlayer() {
        if (!quickLookView.player)
            return
        if (quickLookView.shown && quickLookView.qlIsPlayable
                && quickLookView.qlPath.length > 0)
            quickLookView.player.requestPreview(quickLookView.qlPath)
        else
            quickLookView.player.close()
    }

    readonly property string qlText: (owner.quickLookOpen && qlKind !== "directory"
                                      && !qlIsImage && qlPath.length > 0)
                                     ? controller.previewText(qlPath) : ""
    readonly property bool qlHasText: qlText.length > 0

    // Per-segment encode so spaces / #, ? etc. in a name survive the
    // file:// URL without mangling the path separators.
    function fileUrl(p) {
        return "file://" + p.split("/").map(encodeURIComponent).join("/")
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
            quickLookView.Window.window.activateEntry(controller, controller.selectedToken)
            event.accepted = true
        }
    }

    GlassCard {
        anchors.centerIn: parent
        width: Math.min(720, owner.width - 64)
        height: Math.min(owner.height - 80, 640)
        backdropSource: quickLookView.panel
        // (not transform-scaled — a scale transform desynced the glass backdrop)
        Accessible.role: Accessible.Dialog
        Accessible.name: "Vista previa"

        MouseArea { anchors.fill: parent }   // clicks on the card don't dismiss

        CelestinaIcon {
            id: qlIcon
            x: 18
            y: 16
            width: CelestinaTheme.iconSm
            height: CelestinaTheme.iconSm
            name: panel.mediaIconName(quickLookView.qlKind, quickLookView.qlMedia, quickLookView.qlPath)
            fallbackName: quickLookView.qlKind === "directory" ? "folder" : "file"
            tone: panel.entryIconTone(quickLookView.qlKind)
            tintOverride: panel.iconTint(quickLookView.qlPath)
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

            // (2) Media — el reproductor incrustado. Sólo audio por ahora, y
            // sólo cuando alguien lo pidió: abrir la carpeta no construye nada.
            MediaPreview {
                anchors.fill: parent
                visible: quickLookView.qlIsPlayable
                player: quickLookView.player
                path: quickLookView.qlPath
            }

            // (3) Text / code
            //
            // Composed from the same pieces as Grafita's editing surface — a
            // Flickable, the shared gutter and the shared scroll bars — rather
            // than from a `ScrollView`. A preview whose whole purpose is to
            // let you read a file should number its lines like the editor that
            // opens it, and the two surfaces now differ only in that this one
            // cannot be typed into.
            Item {
                id: preview
                anchors.fill: parent
                visible: !quickLookView.qlIsImage && quickLookView.qlHasText

                CelestinaLineGutter {
                    id: previewGutter
                    anchors.left: parent.left
                    // The text is set in from the card rather than run against
                    // it, and the first column needs somewhere to breathe.
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    surface: previewText
                    viewportY: previewScroller.contentY
                    viewportHeight: previewScroller.height
                }

                Flickable {
                    id: previewScroller
                    // A gap wide enough that a number and its line do not read
                    // as one string.
                    anchors.left: previewGutter.right
                    anchors.leftMargin: CelestinaTheme.spaceMd
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: parent.bottom
                    anchors.rightMargin: CelestinaTheme.spaceSm
                    clip: true
                    contentWidth: previewText.width
                    contentHeight: previewText.paintedHeight
                    boundsBehavior: Flickable.StopAtBounds

                    TextEdit {
                        id: previewText
                        // Unwrapped, so the shape of code survives the peek;
                        // the surface is as wide as its longest line, which is
                        // what the horizontal bar below scrolls through.
                        width: Math.max(previewScroller.width, implicitWidth)
                        readOnly: true
                        text: quickLookView.qlText
                        wrapMode: TextEdit.NoWrap
                        selectByMouse: true
                        color: CelestinaTheme.text
                        selectionColor: CelestinaTheme.accent
                        selectedTextColor: CelestinaTheme.accentInk
                        font.family: CelestinaTheme.monoFamily
                        font.pixelSize: quickLookView.reading.fontSize

                        Accessible.role: Accessible.StaticText
                        Accessible.name: quickLookView.qlText
                    }
                }

                CelestinaScrollBar {
                    surface: previewScroller
                    anchors.right: previewScroller.right
                    anchors.top: previewScroller.top
                    anchors.bottom: previewScroller.bottom
                    anchors.bottomMargin: previewSideways.visible
                                          ? previewSideways.height : 0
                }

                CelestinaScrollBar {
                    id: previewSideways
                    horizontal: true
                    surface: previewScroller
                    anchors.left: previewScroller.left
                    anchors.right: previewScroller.right
                    anchors.bottom: previewScroller.bottom
                }
            }

            // (4) No renderable preview — a centred glyph + reason.
            Column {
                anchors.centerIn: parent
                spacing: 12
                visible: !quickLookView.qlIsImage && !quickLookView.qlIsPlayable
                         && !quickLookView.qlHasText
                CelestinaIcon {
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: 56
                    height: 56
                    name: panel.mediaIconName(quickLookView.qlKind,
                                                  quickLookView.qlMedia, quickLookView.qlPath)
                    sourceSize: Qt.size(width, height)
                    fallbackName: quickLookView.qlKind === "directory" ? "folder" : "file"
                    tone: panel.entryIconTone(quickLookView.qlKind)
                    tintOverride: panel.iconTint(quickLookView.qlPath)
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    horizontalAlignment: Text.AlignHCenter
                    text: quickLookView.qlKind === "directory" ? "Carpeta"
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
            opacity: CelestinaTheme.mutedContentOpacity
        }
    }
}
