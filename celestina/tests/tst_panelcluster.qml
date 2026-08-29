import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop
import "../qml/EdgeAttachedGeometry.js" as EdgeAttachedGeometry

// A semantic cluster owns one dense material capsule for all of its controls.
// The continuous panel backdrop owns the compositor sample, so neither the
// cluster nor its child buttons publish another blur region.
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

    SignalSpy {
        id: firstButtonRequests

        target: firstButton
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
            if (node.objectName === "celestina-panel-tint")
                found.push(node);

            for (let index = 0; index < node.children.length; ++index)
                visit(node.children[index]);
        }

        visit(item);
        return found;
    }

    function test_a_populated_cluster_has_one_shared_material_capsule() {
        verify(cluster.hasContent);
        compare(cluster.spacing, CelestinaTheme.spaceXs);
        compare(firstButton.ownsGlass, false);
        compare(secondButton.ownsGlass, false);
        // SIMPLE-1: the bar's continuous veil is gone; each visible pill
        // publishes its own capsule region for the strong colour summary.
        compare(testCase.glassRegions(cluster).length, 3);
        compare(testCase.visibleGlassRegions(cluster).length, 1);
        const pillMaterials = testCase.materials(cluster);
        compare(pillMaterials.length, 3);
        let visibleMaterials = 0;
        for (let index = 0; index < pillMaterials.length; ++index) {
            const material = pillMaterials[index];
            if (material.visible)
                visibleMaterials += 1;
            // SIMPLE-1: the pill's material is the MenuSection card — a
            // flat mica tint, no ornamented glass surface left.
            compare(material.radius, CelestinaTheme.radiusPill);
            compare(material.color,
                    Qt.rgba(CelestinaTheme.elevated.r,
                            CelestinaTheme.elevated.g,
                            CelestinaTheme.elevated.b, 0.55));
            // The tint sits inside the ShellPanel, which fills the pill.
            compare(material.parent.parent.height,
                    CelestinaTheme.controlHeightXs);
            compare(material.parent.parent.horizontalOverhang,
                    CelestinaTheme.spaceSm);
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

    function test_the_edge_to_edge_bar_keeps_its_lower_stroke_open() {
        const shape = EdgeAttachedGeometry.openBottomRectangle(1920, 40);
        compare(shape.path,
                "M 0 0 L 1920 0 L 1920 40 L 0 40 Z");
        compare(shape.edgePath,
                "M 0 40 L 0 0 L 1920 0 L 1920 40");
    }

    function test_a_menu_request_anchors_at_the_icon_without_deforming_the_capsule() {
        const material = testCase.materials(cluster).filter(
            candidate => candidate.visible)[0];
        verify(material);
        const pill = material.parent;
        const pillGeometry = Qt.rect(pill.x, pill.y, pill.width, pill.height);
        const firstGeometry = Qt.rect(firstButton.x, firstButton.y,
                                      firstButton.width, firstButton.height);
        const secondGeometry = Qt.rect(secondButton.x, secondButton.y,
                                       secondButton.width, secondButton.height);
        const cornerRadius = material.radius;

        firstButtonRequests.clear();
        firstButton.requestMenu();
        compare(firstButtonRequests.count, 1);
        const opener = firstButtonRequests.signalArguments[0][0];
        const anchor = firstButtonRequests.signalArguments[0][1];
        const expectedAnchor = firstButton.attachmentAnchorGlobalRectNow();

        compare(opener.width, firstButton.width);
        compare(opener.height, firstButton.height);
        compare(anchor, expectedAnchor);
        compare(anchor.width, 18);
        compare(anchor.height, 18);
        compare(anchor.x, opener.x + (opener.width - anchor.width) / 2);
        compare(anchor.y, opener.y + (opener.height - anchor.height) / 2);
        verify(firstButton.isPanelAttachmentSource);

        compare(Qt.rect(pill.x, pill.y, pill.width, pill.height), pillGeometry);
        compare(pill.height, CelestinaTheme.controlHeightXs);
        compare(material.radius, cornerRadius);
        compare(Qt.rect(firstButton.x, firstButton.y,
                        firstButton.width, firstButton.height), firstGeometry);
        compare(Qt.rect(secondButton.x, secondButton.y,
                        secondButton.width, secondButton.height), secondGeometry);

        // The surface lease may keep the exact opener's local hover circle,
        // but that feedback belongs to the button background only. It never
        // opens, stretches or recolours the shared dense capsule.
        firstButton.menuOpen = true;
        tryCompare(firstButton.background, "color", testInk.controlFill);
        compare(secondButton.background.color, CelestinaTheme.clear);
        compare(Qt.rect(pill.x, pill.y, pill.width, pill.height), pillGeometry);
        // SIMPLE-1: the pill's material is the MenuSection card — a flat
        // mica tint, no ornamented glass surface left.
        compare(material.radius, cornerRadius);
        compare(material.color,
                Qt.rgba(CelestinaTheme.elevated.r, CelestinaTheme.elevated.g,
                        CelestinaTheme.elevated.b, 0.55));
        firstButton.menuOpen = false;
    }

}
