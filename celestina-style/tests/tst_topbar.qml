import QtQuick
import QtQuick.Window
import QtTest
import CelestinaStyle

// The fixed bar every application wears. Asserted: its height and canvas
// fill; the title left-aligned right after the leading slot and eliding; the
// trailing slot right-aligned; the tool bar role; that it takes no focus of
// its own while its buttons keep theirs.
// language-contract: allow-non-english
TestCase {
    id: testCase

    name: "CelestinaTopBar"
    when: testWindow.visible

    Window {
        id: testWindow
        width: 640
        height: 200
        visible: true

        Item { id: before; width: 10; height: 10; activeFocusOnTab: true }

        CelestinaTopBar {
            id: bar
            width: 640
            title: "Un título tan largo que no cabe en la anchura que le queda entre los dos huecos de la barra"
            subtitle: "AMD Ryzen 7"
            leadingData: [
                CelestinaSegmentedControl {
                    id: segments
                    helpText: "Secciones"
                    model: [ { key: "a", icon: "gauge", label: "Rendimiento" }, { key: "b", icon: "view-list", label: "Procesos" } ]
                }
            ]
            trailingData: [
                CelestinaIconButton { id: firstAction; iconName: "search"; helpText: "Buscar"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd },
                CelestinaIconButton { id: secondAction; iconName: "view-grid"; helpText: "Vista"; role: CelestinaButton.Ghost; iconSize: CelestinaTheme.iconMd }
            ]
        }

        CelestinaTopBar {
            id: bar2
            y: 100
            width: 640
            title: "Ajustes"
        }
    }

    function init() {
        testWindow.requestActivate()
        tryCompare(testWindow, "active", true)
        before.forceActiveFocus(Qt.MouseFocusReason)
        tryCompare(before, "activeFocus", true)
    }

    function test_title_starts_at_the_margin_without_leading() {
        const title = findChild(bar2, "topBarTitle")
        verify(title)
        compare(title.mapToItem(bar2, 0, 0).x, CelestinaTheme.windowMargin)
    }

    function test_height_and_fill() {
        compare(bar.implicitHeight, CelestinaTheme.topBarHeight)
        compare(bar.height, CelestinaTheme.topBarHeight)
        compare(bar.background.color, CelestinaTheme.canvas)
        compare(bar.leftPadding, CelestinaTheme.windowMargin)
        compare(bar.rightPadding, CelestinaTheme.windowMargin)
    }

    function test_title_sits_left_after_the_leading_slot_and_elides() {
        const leading = findChild(bar, "topBarLeading")
        const title = findChild(bar, "topBarTitle")
        const subtitle = findChild(bar, "topBarSubtitle")
        verify(leading && title && subtitle)
        compare(leading.x, 0)
        compare(title.horizontalAlignment, Text.AlignLeft)
        compare(title.elide, Text.ElideRight)
        verify(title.truncated)
        compare(title.font.pixelSize, CelestinaTheme.fontTitle)
        compare(subtitle.font.pixelSize, CelestinaTheme.fontRowSecondary)
        verify(title.mapToItem(bar, 0, 0).x >= leading.width + CelestinaTheme.windowMargin)
    }

    function test_trailing_sits_right() {
        const trailing = findChild(bar, "topBarTrailing")
        verify(trailing)
        compare(trailing.mapToItem(bar, trailing.width, 0).x, bar.width - CelestinaTheme.windowMargin)
        compare(secondAction.x, firstAction.x + firstAction.width + CelestinaTheme.spaceXs)
        compare(firstAction.iconSize, CelestinaTheme.iconMd)
    }

    function test_role_and_focus() {
        compare(bar.Accessible.role, Accessible.ToolBar)
        compare(bar.Accessible.name, bar.title)
        verify(!bar.activeFocusOnTab)
        keyClick(Qt.Key_Tab)
        tryCompare(findChild(segments, "segment-0"), "activeFocus", true)
        keyClick(Qt.Key_Tab)
        tryCompare(firstAction, "activeFocus", true)
    }
}
