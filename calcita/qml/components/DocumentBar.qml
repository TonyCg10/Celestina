import QtQuick
import org.celestina.calcita 1.0

// The bar over the pages, a glass pill: the document's name, the page field
// «n / N» (type a page, +n, -n, «inicio» or «fin» and press Enter), the zoom
// controls, the search, outline and reading-mode toggles and «Abrir…». It only reports what the person asked; the window
// hands each request to its `CalcitaDocument`.
Item {
    id: bar

    property string name: ""
    property int page: 0
    property int pageCount: 0
    property string zoomMode: "fitWidth"
    property real zoomFactor: 1
    property bool searchOpen: false
    property bool outlineOpen: false
    property bool readingDark: false

    signal goToRequested(string text)
    signal zoomInRequested()
    signal zoomOutRequested()
    signal fitWidthRequested()
    signal fitPageRequested()
    signal openRequested()
    signal searchToggled()
    signal outlineToggled()
    signal readingToggled()
    // Enter or Escape in the page field: focus goes back to the pages.
    signal fieldDone()

    readonly property string pageDisplay: bar.pageCount > 0
                                          ? bar.page + " / " + bar.pageCount
                                          : "– / –"

    // Ctrl+G: the field takes the focus with its text selected.
    function focusPageField() {
        pageField.forceActiveFocus()
        pageField.selectAll()
    }

    function showPage() {
        if (!pageField.activeFocus)
            pageField.text = bar.pageDisplay
    }

    onPageDisplayChanged: bar.showPage()

    objectName: "documentBar"
    Accessible.role: Accessible.ToolBar
    Accessible.name: qsTr("Barra del documento")
    implicitWidth: row.implicitWidth + CelestinaTheme.spaceSm * 2
    implicitHeight: row.implicitHeight + CelestinaTheme.spaceXs * 2

    CelestinaSurface {
        anchors.fill: parent
        role: CelestinaSurface.Elevated
        Accessible.ignored: true
        radiusOverride: CelestinaTheme.radiusPill
    }

    Row {
        id: row
        anchors.centerIn: parent
        spacing: CelestinaTheme.spaceSm

        Text {
            objectName: "documentName"
            anchors.verticalCenter: parent.verticalCenter
            width: Math.min(implicitWidth, 240)
            leftPadding: CelestinaTheme.spaceSm
            elide: Text.ElideMiddle
            text: bar.name
            color: CelestinaTheme.text
            font.pixelSize: CelestinaTheme.fontBody
            font.weight: CelestinaTheme.weightDemiBold
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        CelestinaTextField {
            id: pageField
            objectName: "pageField"
            anchors.verticalCenter: parent.verticalCenter
            width: 92
            height: CelestinaTheme.controlHeightSm
            horizontalAlignment: TextInput.AlignHCenter
            font.features: CelestinaTheme.fontFeaturesTabular
            Accessible.name: bar.pageCount > 0
                             ? qsTr("Página %1 de %2").arg(bar.page).arg(bar.pageCount)
                             : qsTr("Página")
            Accessible.description: qsTr("Escribe un número de página, +n, -n, inicio o fin")
            Component.onCompleted: bar.showPage()
            onActiveFocusChanged: {
                if (activeFocus)
                    selectAll()
                else
                    bar.showPage()
            }
            onAccepted: {
                bar.goToRequested(text)
                bar.fieldDone()
            }
            Keys.onEscapePressed: bar.fieldDone()
        }

        CelestinaCapsule {
            anchors.verticalCenter: parent.verticalCenter

            CelestinaIconButton {
                objectName: "zoomOutButton"
                role: CelestinaButton.Ghost
                iconName: "minus"
                helpText: qsTr("Alejar")
                onClicked: bar.zoomOutRequested()
            }

            Text {
                objectName: "zoomLabel"
                anchors.verticalCenter: parent.verticalCenter
                width: 52
                horizontalAlignment: Text.AlignHCenter
                text: Math.round(bar.zoomFactor * 100) + " %"
                color: CelestinaTheme.textMuted
                font.pixelSize: CelestinaTheme.fontCaption
                font.features: CelestinaTheme.fontFeaturesTabular
                Accessible.role: Accessible.StaticText
                Accessible.name: qsTr("Zoom %1 %").arg(Math.round(bar.zoomFactor * 100))
            }

            CelestinaIconButton {
                objectName: "zoomInButton"
                role: CelestinaButton.Ghost
                iconName: "zoom-in"
                helpText: qsTr("Acercar")
                onClicked: bar.zoomInRequested()
            }
        }

        CelestinaButton {
            objectName: "fitWidthButton"
            anchors.verticalCenter: parent.verticalCenter
            role: bar.zoomMode === "fitWidth" ? CelestinaButton.Selected : CelestinaButton.Ghost
            text: qsTr("Ancho")
            helpText: qsTr("Ajustar al ancho")
            onClicked: bar.fitWidthRequested()
        }

        CelestinaButton {
            objectName: "fitPageButton"
            anchors.verticalCenter: parent.verticalCenter
            role: bar.zoomMode === "fitPage" ? CelestinaButton.Selected : CelestinaButton.Ghost
            text: qsTr("Página")
            helpText: qsTr("Ajustar a la página")
            onClicked: bar.fitPageRequested()
        }

        CelestinaIconButton {
            objectName: "searchButton"
            anchors.verticalCenter: parent.verticalCenter
            role: bar.searchOpen ? CelestinaButton.Selected : CelestinaButton.Ghost
            iconName: "search"
            helpText: qsTr("Buscar")
            onClicked: bar.searchToggled()
        }

        CelestinaIconButton {
            objectName: "outlineButton"
            anchors.verticalCenter: parent.verticalCenter
            role: bar.outlineOpen ? CelestinaButton.Selected : CelestinaButton.Ghost
            iconName: "view-list"
            helpText: qsTr("Índice")
            onClicked: bar.outlineToggled()
        }

        CelestinaIconButton {
            objectName: "readingButton"
            anchors.verticalCenter: parent.verticalCenter
            role: bar.readingDark ? CelestinaButton.Selected : CelestinaButton.Ghost
            iconName: "sun"
            helpText: bar.readingDark ? qsTr("Lectura clara") : qsTr("Lectura oscura")
            Accessible.checkable: true
            Accessible.checked: bar.readingDark
            onClicked: bar.readingToggled()
        }

        CelestinaIconButton {
            objectName: "barOpenButton"
            anchors.verticalCenter: parent.verticalCenter
            role: CelestinaButton.Ghost
            iconName: "folder-open"
            helpText: qsTr("Abrir…")
            onClicked: bar.openRequested()
        }
    }
}
