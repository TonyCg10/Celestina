// language-contract: allow-non-english
import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

TestCase {
    id: testCase

    name: "ListSection"
    when: testWindow.visible

    Window {
        id: testWindow
        width: 320
        height: 240
        visible: true

        ListSection {
            id: section
            width: 280
            title: "Sección"
            Rectangle { objectName: "row"; width: parent.width; height: CelestinaTheme.rowHeight; radius: CelestinaTheme.radiusMd }
        }
    }

    function test_rows_sit_one_card_inset_inside_the_card() {
        const row = findChild(section, "row")
        verify(row)
        const holder = row.parent
        compare(holder.y, CelestinaTheme.spaceCardInset)
        compare(holder.x, CelestinaTheme.spaceCardInset)
        compare(holder.width, holder.parent.width - CelestinaTheme.spaceCardInset * 2)
        // Concentric: card radius = row radius + inset.
        compare(CelestinaTheme.radiusLg, CelestinaTheme.radiusMd + CelestinaTheme.spaceCardInset)
    }
}
