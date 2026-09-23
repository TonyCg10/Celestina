pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import org.celestina.hematita 1.0

// What is known about the entry under the cursor: its name and path, its
// size on disk and as files report it, how many files it holds and, for a
// verified duplicate, where its identical copies are. The page composes the
// values; names and paths are the filesystem's own.
CelestinaSurface {
    id: card

    // { name, path, allocated, apparent, files, copies: [path, ...] }
    required property var details

    role: CelestinaSurface.Panel
    padding: CelestinaTheme.spaceMd

    Accessible.role: Accessible.Grouping
    Accessible.name: qsTr("Detalles de %1").arg(card.details.name)

    contentItem: ColumnLayout {
        spacing: CelestinaTheme.spaceXs

        Text {
            Layout.fillWidth: true
            text: card.details.name
            textFormat: Text.PlainText
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontRowTitle
            elide: Text.ElideMiddle
        }

        Text {
            Layout.fillWidth: true
            text: card.details.path
            textFormat: Text.PlainText
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            elide: Text.ElideLeft
        }

        Text {
            Layout.fillWidth: true
            text: qsTr("En disco: %1 · Tamaño: %2 · %3 archivos").arg(card.details.allocated)
                                                                  .arg(card.details.apparent)
                                                                  .arg(card.details.files)
            color: CelestinaTheme.textMuted
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
            font.features: CelestinaTheme.fontFeaturesTabular
            wrapMode: Text.WordWrap
        }

        Text {
            Layout.fillWidth: true
            visible: card.details.copies.length > 0
            text: qsTr("Copias idénticas")
            color: CelestinaTheme.text
            font.family: CelestinaTheme.sansFamily
            font.pixelSize: CelestinaTheme.fontCaption
        }

        Repeater {
            model: card.details.copies.length

            delegate: Text {
                id: copy

                required property int index

                Layout.fillWidth: true
                text: card.details.copies[copy.index]
                textFormat: Text.PlainText
                color: CelestinaTheme.textMuted
                font.family: CelestinaTheme.sansFamily
                font.pixelSize: CelestinaTheme.fontCaption
                elide: Text.ElideLeft
            }
        }
    }
}
