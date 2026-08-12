import QtQuick
import QtTest
import CelestinaStyle
import "../qml" as Desktop

// A permanent panel entry point must remain a real button and may contribute
// its dense capsule without owning either the shared bar blur or the overlay
// it opens.
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
        const openerRect = menus.signalArguments[0][0];
        const anchorRect = menus.signalArguments[0][1];
        compare(openerRect.width, button.width);
        compare(openerRect.height, button.height);
        compare(anchorRect, button.attachmentAnchorGlobalRectNow());
        compare(anchorRect.width, 18);
        compare(anchorRect.height, 18);
        compare(anchorRect.x,
                openerRect.x + (openerRect.width - anchorRect.width) / 2);
        compare(anchorRect.y,
                openerRect.y + (openerRect.height - anchorRect.height) / 2);
        verify(button.isPanelAttachmentSource);
    }

    function test_owned_and_delegated_buttons_keep_distinct_material_visibility() {
        compare(button.ownsGlass, true);
        const standaloneRegions = testCase.glassRegions(button);
        compare(standaloneRegions.length, 0);
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
        verify(!standaloneMaterial.usesSilhouette);
        compare(standaloneMaterial.silhouettePath, "");
        compare(standaloneMaterial.silhouetteEdgePath, "");
        verify(standaloneMaterial.materialEdgesVisible);

        compare(groupedButton.ownsGlass, false);
        const groupedRegions = testCase.glassRegions(groupedButton);
        compare(groupedRegions.length, 0);
        const groupedMaterial = testCase.material(groupedButton);
        verify(groupedMaterial);
        verify(!groupedMaterial.visible);
        compare(groupedMaterial.captureActive, false);
    }
}
