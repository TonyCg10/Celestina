import QtQuick
import org.celestina.siderita 1.0

Item {
    id: root

    required property int index
    required property string name
    required property string token
    required property string kind
    required property string path
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
            isDirectory ? "folder"
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
        anchors.margins: 4
        radius: CelestinaTheme.radiusSm
        color: root.chosen ? CelestinaTheme.surfaceSelected
               : root.interactive && cellMouse.containsMouse
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

        anchors.horizontalCenter: parent.horizontalCenter
        y: 10
        width: Math.round(72 * root.iconScale)
        height: width
        radius: CelestinaTheme.radiusSm
        clip: true
        color: CelestinaTheme.clear

        CelestinaIcon {
            anchors.centerIn: parent
            visible: !preview.ready
            width: Math.round(54 * root.iconScale)
            height: width
            sourceSize: Qt.size(width, width)
            name: root.iconName
            fallbackName: root.isDirectory ? "folder"
                          : root.kind === "symlink" ? "symlink" : "file"
            tone: root.isDirectory ? CelestinaIcon.Folder
                  : root.kind === "symlink" ? CelestinaIcon.Symlink
                  : CelestinaIcon.File
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
                    ? "image://thumb/" + encodeURIComponent(root.path) : ""
            sourceSize: Qt.size(256, 256)
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

    Text {
        anchors.horizontalCenter: parent.horizontalCenter
        y: tile.y + tile.height + 8
        width: parent.width - 14
        horizontalAlignment: Text.AlignHCenter
        text: root.name
        color: CelestinaTheme.text
        font.family: CelestinaTheme.sansFamily
        font.pixelSize: Math.round(CelestinaTheme.fontCaption * root.textScale)
        elide: Text.ElideRight
        maximumLineCount: 2
        wrapMode: Text.Wrap
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
