import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property int index
    required property string name
    required property string token
    required property string kind
    required property string path
    // The row's two secondary values arrive through named model roles just as
    // `name` and `path` do.
    required property string sizeText
    required property string dateText
    required property int cellWidth
    required property int cellHeight
    required property real iconScale
    required property real textScale
    required property bool eligible
    required property bool navigable
    required property bool chosen
    required property bool currentItem
    required property bool focusVisible
    required property bool banding
    required property Item contentItem
    // Folder-type/tone rules shared with the main folder view
    // (PickerIconRules.qml), so a folder here looks like the one already known.
    required property var iconRules

    readonly property bool isDirectory: kind === "directory"
    readonly property bool hidden: name.charAt(0) === "."
    readonly property bool interactive: eligible || navigable
    readonly property string mediaKind:
            /\.(png|jpe?g|gif|webp|bmp|ico|tiff?|avif|jxl|heic|heif)$/i.test(name)
            ? "image"
          : /\.(mp4|mkv|webm|mov|avi|m4v|mpe?g|wmv|flv|3gp|ogv|ts)$/i.test(name)
            ? "video"
          : /\.(mp3|flac|ogg|oga|opus|m4a|aac|wav|wma|aiff?|mka)$/i.test(name)
            ? "audio" : ""
    readonly property bool previewable: !isDirectory && mediaKind === "image"
    readonly property string iconName:
            isDirectory ? root.iconRules.folderIcon(root.path)
          : kind === "symlink" ? "emblem-symbolic-link"
          : mediaKind === "image" ? "image-x-generic"
          : mediaKind === "video" ? "video-x-generic"
          : mediaKind === "audio" ? "audio-x-generic"
          : "text-x-generic"

    signal bandBeginRequested(real x, real y, int modifiers)
    signal bandUpdateRequested(real x, real y)
    signal bandFinishRequested
    signal cellClicked(int modifiers)
    signal cellActivated

    width: cellWidth
    height: cellHeight
    opacity: !interactive ? CelestinaTheme.unavailableContentOpacity
             : hidden ? CelestinaTheme.disabledOpacity : 1
    Accessible.role: Accessible.ListItem
    Accessible.name: name
    Accessible.description: !interactive ? "No se puede elegir"
                            : hidden ? "Elemento oculto"
                            : navigable && !eligible ? "Carpeta navegable"
                            : ""
    Accessible.selected: chosen
    Accessible.focusable: interactive
    Accessible.focused: currentItem && focusVisible
    Accessible.onPressAction: if (root.interactive) root.cellActivated()

    Rectangle {
        anchors.fill: parent
        anchors.topMargin: 1
        anchors.bottomMargin: 1
        radius: CelestinaTheme.radiusSm
        // Draw the current row even when it cannot be chosen. A folder is not
        // a valid answer to an ordinary file request, so otherwise clicking it
        // appeared to do nothing. Chosen and current remain distinct: the
        // former uses selection fill, the latter the hover tone.
        color: root.chosen ? CelestinaTheme.surfaceSelected
               : (root.interactive
                  && (cellMouse.containsMouse || root.currentItem))
                 ? CelestinaTheme.surfaceHover : CelestinaTheme.clear
        border.width: root.focusVisible ? CelestinaTheme.borderFocus
                      : root.chosen ? CelestinaTheme.borderHairline : 0
        border.color: root.focusVisible ? CelestinaTheme.focusRing
                                        : CelestinaTheme.dividerStrong

        Behavior on color {
            ColorAnimation {
                duration: CelestinaTheme.reducedMotion
                          ? 0 : CelestinaTheme.motionFast
            }
        }
    }

    Rectangle {
        id: tile

        x: 10
        anchors.verticalCenter: parent.verticalCenter
        width: Math.round(CelestinaTheme.iconSm * root.iconScale)
        height: width
        radius: CelestinaTheme.radiusSm
        clip: true
        color: CelestinaTheme.clear

        EntryGlyph {
            anchors.centerIn: parent
            visible: !preview.ready
            width: parent.width
            height: width
            kind: root.kind
            path: root.path
            iconName: root.iconName
            fallbackName: root.isDirectory ? "folder"
                          : root.kind === "symlink" ? "symlink" : "file"
            tone: root.iconRules.entryIconTone(root.kind)
        }

        Image {
            id: preview

            anchors.fill: parent
            anchors.margins: 1
            readonly property bool ready: root.previewable
                                          && status === Image.Ready
            visible: opacity > 0
            opacity: ready ? 1 : 0
            source: root.previewable
                    ? "image://thumb/" + root.path : ""
            sourceSize: Qt.size(64, 64)
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            cache: true
            smooth: true

            Behavior on opacity {
                NumberAnimation {
                    duration: CelestinaTheme.reducedMotion
                              ? 0 : CelestinaTheme.motionNormal
                }
            }
        }
    }

    // Measure the date from the right first and give the remaining width to
    // the name, so a long name elides instead of pushing dates outside the row.
    Text {
        id: dateLabel

        anchors.right: parent.right
        anchors.rightMargin: 12
        anchors.verticalCenter: parent.verticalCenter
        // Visibility follows row width rather than this label's own position;
        // using `x` here would create a self-referential binding.
        visible: root.dateText.length > 0 && root.width > 340
        text: root.dateText
        color: CelestinaTheme.textMuted
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.textScale)
        elide: Text.ElideRight
    }

    Column {
        x: tile.x + tile.width + 10
        anchors.verticalCenter: parent.verticalCenter
        width: Math.max(0, (dateLabel.visible ? dateLabel.x : root.width - 12)
                           - x - 12)
        spacing: 0

        Text {
            width: parent.width
            text: root.name
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontBody * root.textScale)
            elide: Text.ElideRight
            maximumLineCount: 1
        }

        Text {
            width: parent.width
            // The details view uses a dash when a folder has no size. In this
            // compact row it is noise, so preserve the row height but hide it.
            visible: text.length > 0 && text !== "—"
            text: root.sizeText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.textScale)
            elide: Text.ElideRight
            maximumLineCount: 1
        }
    }

    MouseArea {
        id: cellMouse

        anchors.fill: parent
        hoverEnabled: true
        preventStealing: root.banding
        cursorShape: root.interactive ? Qt.PointingHandCursor : Qt.ArrowCursor

        property bool armed: false
        property real pressX: 0
        property real pressY: 0

        function toContent(mouse) {
            return cellMouse.mapToItem(root.contentItem, mouse.x, mouse.y)
        }

        onPressed: function(mouse) {
            const point = toContent(mouse)
            pressX = point.x
            pressY = point.y
            armed = true
        }
        onPositionChanged: function(mouse) {
            if (!armed)
                return
            const point = toContent(mouse)
            if (!root.banding
                    && (Math.abs(point.x - pressX) > 5
                        || Math.abs(point.y - pressY) > 5))
                root.bandBeginRequested(pressX, pressY, mouse.modifiers)
            root.bandUpdateRequested(point.x, point.y)
        }
        onReleased: {
            armed = false
            root.bandFinishRequested()
        }
        onCanceled: {
            armed = false
            root.bandFinishRequested()
        }
        onClicked: function(mouse) {
            root.cellClicked(mouse.modifiers)
        }
        onDoubleClicked: if (root.interactive) root.cellActivated()
    }
}
