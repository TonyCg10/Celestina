import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// A semantic cluster owns one material region for all of its controls. The
// controls still own their interaction, but must delegate glass to the group
// or overlapping blur regions turn the compact cluster back into dense pills.
TestCase {
    id: testCase

    name: "PanelCluster"
    visible: true
    when: windowShown
    width: 240
    height: 80

    Desktop.BackdropInk {
        id: testInk
    }

    Desktop.PanelCluster {
        id: cluster

        anchors.top: parent.top
        blurAvailable: true
        ink: testInk
        spacing: CelestinaTheme.spaceXs

        Desktop.PanelActionButton {
            id: firstButton

            ink: testInk
            blurAvailable: true
            ownsGlass: false
            iconName: "bell"
            helpText: qsTr("Abrir notificaciones")
        }

        Desktop.PanelActionButton {
            id: secondButton

            ink: testInk
            blurAvailable: true
            ownsGlass: false
            iconName: "settings"
            helpText: qsTr("Abrir el centro de control")
        }
    }

    Desktop.PanelCluster {
        id: emptyCluster

        anchors.bottom: parent.bottom
        blurAvailable: true
        ink: testInk
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

    function visibleGlassRegions(item) {
        const regions = testCase.glassRegions(item);
        const visible = [];
        for (let index = 0; index < regions.length; ++index) {
            if (regions[index].visible)
                visible.push(regions[index]);
        }
        return visible;
    }

    function materials(item) {
        const found = [];

        function visit(node) {
            if (node.objectName === "celestina-panel-pill-material")
                found.push(node);

            for (let index = 0; index < node.children.length; ++index)
                visit(node.children[index]);
        }

        visit(item);
        return found;
    }

    function test_a_populated_cluster_has_one_shared_glass_region() {
        verify(cluster.hasContent);
        compare(cluster.spacing, CelestinaTheme.spaceXs);
        compare(firstButton.ownsGlass, false);
        compare(secondButton.ownsGlass, false);
        compare(testCase.glassRegions(cluster).length, 3);
        compare(testCase.visibleGlassRegions(cluster).length, 1);
        const pillMaterials = testCase.materials(cluster);
        compare(pillMaterials.length, 3);
        let visibleMaterials = 0;
        for (let index = 0; index < pillMaterials.length; ++index) {
            const material = pillMaterials[index];
            if (material.visible)
                visibleMaterials += 1;
            compare(material.backdropMode, GlassSurface.ExternalBackdrop);
            compare(material.captureActive, false);
            compare(material.materialRole, GlassSurface.ContentSurface);
            compare(material.materialStrength,
                    CelestinaTheme.glassContentSurfaceStrength);
            compare(material.materialTint, testInk.contentMaterialTint);
            compare(material.elevation, 0);
        }
        compare(visibleMaterials, 1);
        compare(cluster.implicitWidth,
                firstButton.implicitWidth + CelestinaTheme.spaceXs
                + secondButton.implicitWidth);
    }

    function test_an_empty_cluster_withdraws_its_region_and_width() {
        verify(!emptyCluster.hasContent);
        compare(emptyCluster.implicitWidth, 0);
        verify(!emptyCluster.visible);
        compare(testCase.visibleGlassRegions(emptyCluster).length, 0);
        compare(testCase.materials(emptyCluster).length, 1);
        verify(!testCase.materials(emptyCluster)[0].visible);
    }
}
