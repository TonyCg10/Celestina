// A folder-backed wallpaper gallery for one output.
//
// The permanent panel owns the native folder chooser. This transient surface
// owns only the bounded catalogue published by the helper and the per-output
// selection request. Keeping those responsibilities separate means reopening
// the gallery never invents a filesystem path in QML, and a thumbnail can be
// chosen without destroying the menu between successive comparisons.
pragma ComponentBehavior: Bound

import CelestinaStyle
import QtQuick
import "ProviderReading.js" as ProviderReading

AnchoredCard {
    id: root

    required property var providerSource
    property alias compositorBlurAvailable: card.compositorBlurAvailable
    property alias glassRects: card.glassRects
    property alias glassRegions: card.glassRegions
    readonly property BackdropInk ink: backdropInk
    signal chooseRequested()

    readonly property var gallery: ProviderReading.read(
                                           root.providerSource,
                                           "wallpaper-gallery")
    readonly property string galleryState: root.gallery !== undefined
                                                   && root.gallery.state !== undefined
                                               ? root.gallery.state : "unconfigured"
    readonly property string folder: root.gallery !== undefined
                                             && root.gallery.folder !== undefined
                                         ? root.gallery.folder : ""
    readonly property string folderUrl: root.gallery !== undefined
                                                && root.gallery.folderUrl !== undefined
                                            ? root.gallery.folderUrl : ""
    readonly property string catalogue: root.gallery !== undefined
                                                && root.gallery.catalogue !== undefined
                                            ? root.gallery.catalogue : ""
    readonly property var images: root.gallery !== undefined
                                          && root.gallery.images !== undefined
                                      ? root.gallery.images : []
    readonly property int page: root.gallery !== undefined
                                     && root.gallery.page !== undefined
                                 ? root.gallery.page : (root.images.length > 0 ? 1 : 0)
    readonly property int pageCount: root.gallery !== undefined
                                          && root.gallery.pageCount !== undefined
                                      ? root.gallery.pageCount
                                      : (root.images.length > 0 ? 1 : 0)
    readonly property int totalImages: root.gallery !== undefined
                                           && root.gallery.total !== undefined
                                       ? root.gallery.total : root.images.length
    readonly property bool hasPreviousPage: root.gallery !== undefined
                                                   && root.gallery.hasPrevious !== undefined
                                               ? root.gallery.hasPrevious
                                               : root.page > 1
    readonly property bool hasNextPage: root.gallery !== undefined
                                               && root.gallery.hasNext !== undefined
                                           ? root.gallery.hasNext
                                           : root.page > 0 && root.page < root.pageCount
    readonly property bool truncated: root.gallery !== undefined
                                             && root.gallery.truncated === true
    readonly property int skipped: root.gallery !== undefined
                                           && root.gallery.skipped !== undefined
                                       ? root.gallery.skipped : 0
    readonly property var ledger: root.providerSource
                                  ? root.providerSource.requests : null
    readonly property string requestTarget: "select:" + root.outputName
    readonly property var requestState: {
        if (!root.ledger || root.ledger.revision < 0)
            return {};
        return root.ledger.stateOf("wallpaper-gallery", root.requestTarget);
    }
    readonly property var folderRequestState: {
        if (!root.ledger || root.ledger.revision < 0)
            return {};
        return root.ledger.stateOf("wallpaper-gallery", "folder");
    }
    readonly property bool hasFolder: root.folder.length > 0
    readonly property bool hasImages: root.images.length > 0

    contentWidth: 424
    contentHeight: 438

    Shortcut {
        sequence: "Escape"
        context: Qt.WindowShortcut
        onActivated: root.dismissed()
    }

    BackdropInk {
        id: backdropInk
    }

    function folderSummary() {
        if (root.folderRequestState.state === "failed")
            return qsTr("No se pudo usar esa carpeta");
        if (!root.hasFolder)
            return qsTr("Elige una carpeta para crear la galería");
        if (root.galleryState === "loading")
            return qsTr("Cargando imágenes…");
        if (root.galleryState === "failed")
            return qsTr("No se pudo leer la carpeta");
        if (root.pageCount > 1) {
            return qsTr("Página %1 de %2 · %n imagen(es)", "", root.totalImages)
                .arg(root.page).arg(root.pageCount);
        }
        return qsTr("%n imagen(es)", "", root.totalImages);
    }

    function chooseFolder() {
        root.chooseRequested();
    }

    function selectImage(image) {
        if (!root.ledger || root.catalogue.length === 0
            || image === undefined || image.id === undefined) {
            return;
        }

        root.ledger.send(
            "wallpaper-gallery",
            "select",
            {
                "output": root.outputName,
                "catalogue": root.catalogue,
                "id": image.id
            },
            root.requestTarget,
            "immediate"
        );
    }

    function showPage(requestedPage) {
        if (!root.ledger || root.catalogue.length === 0
            || requestedPage < 1 || requestedPage > root.pageCount
            || requestedPage === root.page) {
            return;
        }

        root.ledger.send(
            "wallpaper-gallery",
            "set-page",
            {
                "catalogue": root.catalogue,
                "page": requestedPage
            },
            "page",
            "immediate"
        );
    }

    function previousPage() {
        if (root.hasPreviousPage)
            root.showPage(root.page - 1);
    }

    function nextPage() {
        if (root.hasNextPage)
            root.showPage(root.page + 1);
    }

    function activateCurrent() {
        if (!root.hasImages)
            return;
        const index = Math.max(0, Math.min(galleryView.currentIndex,
                                           root.images.length - 1));
        root.selectImage(root.images[index]);
    }

    onReady: {
        card.reveal();
        Qt.callLater(function() {
            if (root.hasImages)
                galleryView.forceActiveFocus();
            else
                folderButton.forceActiveFocus();
        });
    }

    // The full-output carrier makes a click outside the card deterministic.
    // It is declared first so the card's own input stop remains above it.
    Item {
        anchors.fill: parent
        focus: true
        Keys.onEscapePressed: root.dismissed()
        Keys.onDownPressed: {
            if (root.hasImages)
                galleryView.forceActiveFocus();
        }

        MouseArea {
            anchors.fill: parent
            onClicked: root.dismissed()
        }
    }

    SoftOverlayCard {
        id: card

        x: root.cardX
        y: root.cardY
        width: root.contentWidth
        height: root.cardHeight
        reducedMotion: root.reducedMotion
        ink: backdropInk
        accessibleName: qsTr("Fondos de pantalla")
        attachedToTop: root.anchoredFromPanel
        openerRect: root.openerRect
        attachmentAnchorRect: root.attachmentAnchorRect
        attachmentStartY: root.attachmentStartY
        surfacePosition: Qt.point(root.cardX, root.cardY)

        Column {
            anchors.fill: parent
            anchors.margins: CelestinaTheme.spaceMd
            spacing: CelestinaTheme.spaceSm

            MenuHeader {
                width: parent.width
                ink: backdropInk
                title: qsTr("Fondos de pantalla")
                subtitle: root.folderSummary()
                iconName: "image"
            }

            BackdropButton {
                id: folderButton

                objectName: "celestina-wallpaper-folder-button"
                width: parent.width
                height: CelestinaTheme.controlHeight
                ink: backdropInk
                text: qsTr("Elegir carpeta…")
                role: CelestinaButton.Ghost
                activeFocusOnTab: true
                Accessible.name: text
                onClicked: root.chooseFolder()
                Keys.onDownPressed: function(event) {
                    if (root.hasImages) {
                        galleryView.forceActiveFocus();
                        event.accepted = true;
                    }
                }
            }

            Item {
                width: parent.width
                height: parent.height - y

                MenuSection { ink: backdropInk }

                Text {
                    anchors.centerIn: parent
                    width: parent.width - CelestinaTheme.spaceXl * 2
                    visible: !root.hasImages
                    text: root.galleryState === "loading"
                          ? qsTr("Cargando imágenes…")
                          : root.galleryState === "failed"
                            ? qsTr("No se pudo abrir esta carpeta")
                            : root.hasFolder
                              ? qsTr("No hay imágenes compatibles")
                              : qsTr("Elige una carpeta para ver sus imágenes")
                    textFormat: Text.PlainText
                    color: root.galleryState === "failed"
                           ? backdropInk.danger : backdropInk.muted
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontBody
                    horizontalAlignment: Text.AlignHCenter
                    wrapMode: Text.WordWrap
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }

                GridView {
                    id: galleryView

                    objectName: "celestina-wallpaper-gallery"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.top: parent.top
                    anchors.bottom: pageControls.top
                    anchors.margins: CelestinaTheme.spaceSm
                    anchors.bottomMargin: pageControls.visible
                                          ? CelestinaTheme.spaceSm : 0
                    visible: root.hasImages
                    clip: true
                    model: root.images
                    cellWidth: Math.floor(width / 3)
                    cellHeight: 116
                    currentIndex: root.hasImages ? 0 : -1
                    keyNavigationEnabled: true
                    keyNavigationWraps: true
                    boundsBehavior: Flickable.StopAtBounds
                    Accessible.role: Accessible.List
                    Accessible.name: qsTr("Imágenes de fondo")
                    Keys.onEscapePressed: root.dismissed()
                    Keys.onReturnPressed: root.activateCurrent()
                    Keys.onEnterPressed: root.activateCurrent()
                    Keys.onPressed: function(event) {
                        if (event.key === Qt.Key_PageUp) {
                            root.previousPage();
                            event.accepted = true;
                        } else if (event.key === Qt.Key_PageDown) {
                            root.nextPage();
                            event.accepted = true;
                        }
                    }

                    delegate: BackdropButton {
                        id: thumbnail

                        required property int index
                        required property var modelData
                        readonly property bool imageReady:
                            preview.status === Image.Ready

                        objectName: "celestina-wallpaper-thumbnail"
                        width: galleryView.cellWidth - CelestinaTheme.spaceXs
                        height: galleryView.cellHeight - CelestinaTheme.spaceXs
                        ink: backdropInk
                        role: GridView.isCurrentItem
                              ? CelestinaButton.Selected : CelestinaButton.Ghost
                        activeFocusOnTab: true
                        Accessible.name: qsTr("Usar %1 como fondo")
                            .arg(thumbnail.modelData.name)
                        onClicked: {
                            galleryView.currentIndex = index;
                            root.selectImage(thumbnail.modelData);
                        }

                        contentItem: Item {
                            Image {
                                id: preview

                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.top: parent.top
                                anchors.margins: CelestinaTheme.spaceXs
                                height: 72
                                source: thumbnail.modelData.previewUrl
                                sourceSize.width: 180
                                sourceSize.height: 108
                                asynchronous: true
                                cache: true
                                fillMode: Image.PreserveAspectCrop
                                smooth: true
                            }

                            CelestinaIcon {
                                anchors.centerIn: preview
                                width: CelestinaTheme.iconMd
                                height: width
                                visible: !thumbnail.imageReady
                                name: "image"
                                fallbackName: "image"
                                tintOverride: backdropInk.muted
                                Accessible.ignored: true
                            }

                            Text {
                                anchors.left: parent.left
                                anchors.right: parent.right
                                anchors.bottom: parent.bottom
                                anchors.margins: CelestinaTheme.spaceSm
                                text: thumbnail.modelData.name
                                textFormat: Text.PlainText
                                color: backdropInk.primary
                                font.family: CelestinaTheme.sansFamily
                                font.pixelSize: CelestinaTheme.fontCaption
                                horizontalAlignment: Text.AlignHCenter
                                elide: Text.ElideMiddle
                            }
                        }
                    }
                }

                Item {
                    id: pageControls

                    objectName: "celestina-wallpaper-page-controls"
                    anchors.left: parent.left
                    anchors.right: parent.right
                    anchors.bottom: parent.bottom
                    anchors.leftMargin: CelestinaTheme.spaceSm
                    anchors.rightMargin: CelestinaTheme.spaceSm
                    anchors.bottomMargin: CelestinaTheme.spaceSm
                    height: visible ? CelestinaTheme.controlHeight : 0
                    visible: root.pageCount > 1

                    BackdropIconButton {
                        objectName: "celestina-wallpaper-previous-page"
                        anchors.left: parent.left
                        anchors.verticalCenter: parent.verticalCenter
                        width: CelestinaTheme.controlHeight
                        height: width
                        ink: backdropInk
                        iconName: "go-previous"
                        fallbackIcon: "arrow-left"
                        helpText: qsTr("Página anterior")
                        role: CelestinaButton.Ghost
                        enabled: root.hasPreviousPage
                        activeFocusOnTab: true
                        onClicked: root.previousPage()
                    }

                    Text {
                        anchors.centerIn: parent
                        text: qsTr("Página %1 de %2")
                            .arg(root.page).arg(root.pageCount)
                        textFormat: Text.PlainText
                        color: backdropInk.primary
                        font.family: CelestinaTheme.sansFamily
                        font.pixelSize: CelestinaTheme.fontCaption
                        font.weight: CelestinaTheme.weightDemiBold
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }

                    BackdropIconButton {
                        objectName: "celestina-wallpaper-next-page"
                        anchors.right: parent.right
                        anchors.verticalCenter: parent.verticalCenter
                        width: CelestinaTheme.controlHeight
                        height: width
                        ink: backdropInk
                        iconName: "go-next"
                        fallbackIcon: "chevron-right"
                        helpText: qsTr("Página siguiente")
                        role: CelestinaButton.Ghost
                        enabled: root.hasNextPage
                        activeFocusOnTab: true
                        onClicked: root.nextPage()
                    }
                }

                Text {
                    anchors.right: parent.right
                    anchors.bottom: pageControls.visible
                                    ? pageControls.top : parent.bottom
                    anchors.margins: CelestinaTheme.spaceSm
                    visible: root.requestState.state === "pending"
                             || root.requestState.state === "failed"
                    text: root.requestState.state === "pending"
                          ? qsTr("Cambiando…")
                          : qsTr("No se pudo cambiar")
                    textFormat: Text.PlainText
                    color: root.requestState.state === "failed"
                           ? backdropInk.danger : backdropInk.primary
                    font.family: CelestinaTheme.sansFamily
                    font.pixelSize: CelestinaTheme.fontMini
                    font.weight: CelestinaTheme.weightDemiBold
                    z: 2
                }
            }
        }
    }
}
