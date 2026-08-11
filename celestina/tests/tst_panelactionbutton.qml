import QtQuick
import QtTest
import CelestinaStyle
import "../qml" as Desktop

// A permanent panel entry point must remain a real button and must contribute
// its glass region without owning the overlay it opens.
TestCase {
    id: testCase

    name: "PanelActionButton"
    visible: true
    when: windowShown
    width: 120
    height: 40

    Desktop.BackdropInk {
        id: testInk
    }

    Row {
        anchors.centerIn: parent

        Desktop.PanelActionButton {
            id: button

            ink: testInk
            blurAvailable: true
            iconName: "settings"
            helpText: qsTr("Abrir el centro de control")
        }

        Desktop.PanelActionButton {
            id: groupedButton

            ink: testInk
            blurAvailable: true
            ownsGlass: false
            iconName: "app-window"
            helpText: qsTr("Abrir el buscador de aplicaciones")
        }
    }

    SignalSpy {
        id: clicks

        target: button
        signalName: "clicked"
    }

    SignalSpy {
        id: menus

        target: button
        signalName: "menuRequested"
    }

    function glassRegions(item) {
        const found = [];

        function visit(node) {
            if (node.objectName === "celestina-compositor-glass-region")
                found.push(node);

            for (let index = 0; index < node.children.length; ++index)
                visit(node.children[index]);
        }

        visit(item);
        return found;
    }

    function material(item) {
        function visit(node) {
            if (node.objectName === "celestina-panel-pill-material")
                return node;
            for (let index = 0; index < node.children.length; ++index) {
                const found = visit(node.children[index]);
                if (found)
                    return found;
            }
            return null;
        }
        return visit(item);
    }

    function test_it_keeps_one_icon_and_reports_clicks() {
        compare(button.iconName, "settings");
        verify(button.implicitWidth > 0);
        button.click();
        compare(clicks.count, 1);
        compare(menus.count, 1);
        compare(menus.signalArguments[0][2], button.width);
        compare(menus.signalArguments[0][3], button.height);
    }

    function test_standalone_and_grouped_buttons_have_distinct_glass_ownership() {
        compare(button.ownsGlass, true);
        const standaloneRegions = testCase.glassRegions(button);
        compare(standaloneRegions.length, 1);
        verify(standaloneRegions[0].visible);
        const standaloneMaterial = testCase.material(button);
        verify(standaloneMaterial);
        verify(standaloneMaterial.visible);
        compare(standaloneMaterial.backdropMode,
                GlassSurface.ExternalBackdrop);
        compare(standaloneMaterial.captureActive, false);
        compare(standaloneMaterial.materialRole,
                GlassSurface.ContentSurface);
        compare(standaloneMaterial.materialTint,
                testInk.contentMaterialTint);
        compare(standaloneMaterial.elevation, 0);

        compare(groupedButton.ownsGlass, false);
        const groupedRegions = testCase.glassRegions(groupedButton);
        compare(groupedRegions.length, 1);
        verify(!groupedRegions[0].visible);
        const groupedMaterial = testCase.material(groupedButton);
        verify(groupedMaterial);
        verify(!groupedMaterial.visible);
        compare(groupedMaterial.captureActive, false);
    }
}
