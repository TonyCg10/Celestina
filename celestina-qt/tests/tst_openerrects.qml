import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// What each hand-rolled opener actually publishes when clicked.
//
// The drop membrane starts from these two rectangles, and the controller
// refuses an empty opener outright (`addPanelOpenerProperties` returns before
// setting `anchoredFromPanel`), so an opener whose own width is zero opens a
// floating card with no connection to the bar — which is invisible in every
// offscreen test that hands the menu synthetic rectangles, and exactly what
// the author saw live on the clock and the phone.
//
// `PanelMenuButton` descendants inherit working geometry from the control
// anatomy; the clock and the phone reading are plain items that grew their own
// `menuRequested`, so they are the ones this file pins down.
TestCase {
    id: testCase

    name: "OpenerRects"
    when: windowShown
    visible: true
    width: 800
    height: 60

    Desktop.BackdropInk {
        id: testInk
    }

    // The panel's own anatomy: every control lives inside a scene item that
    // carries the per-output factor. What reaches the controller must be real
    // output pixels — mapToGlobal through this transform — because the
    // controller divides by the factor exactly once.
    Item {
        id: scaledScene

        width: testCase.width / 1.15
        height: 40
        transformOrigin: Item.TopLeft
        scale: 1.15
    }

    Component {
        id: clockComponent

        // As the panel really places it: centred, with no explicit size.
        Desktop.Clock {
            anchors.centerIn: parent
            ink: testInk
        }
    }

    Component {
        id: phoneComponent

        Desktop.PhoneStatus {
            anchors.centerIn: parent
            ink: testInk
            blurAvailable: false
            connected: true
            battery: 66
            charging: false
        }
    }

    function openerOf(component) {
        const control = component.createObject(testCase);
        verify(control);
        // The text and layout settle asynchronously; wait for a real width
        // rather than sampling whatever the first frame had.
        tryVerify(function() { return control.implicitWidth > 0; });

        let published = null;
        control.menuRequested.connect(function(opener, anchor) {
            published = {"opener": opener, "anchor": anchor};
        });
        control.requestMenu();
        verify(published !== null);
        control.destroy();
        return published;
    }

    function test_the_clock_publishes_a_real_opener() {
        const rects = testCase.openerOf(clockComponent);
        verify(rects.opener.width > 0,
               "opener width " + rects.opener.width);
        verify(rects.opener.height > 0,
               "opener height " + rects.opener.height);
        verify(rects.anchor.width > 0);
        verify(rects.anchor.height > 0);
        // The anchor is the glyph inside the control, never something wider.
        verify(rects.anchor.width <= rects.opener.width + 0.001);
    }

    // Decides where the live drops land: if this mapping did not include the
    // scene's scale, every hand-rolled opener would publish unscaled numbers,
    // the controller would divide them again, and the membrane would land
    // left of its glyph by the factor — which is what the author photographed.
    function test_an_opener_inside_the_scaled_scene_publishes_real_pixels() {
        const control = clockComponent.createObject(scaledScene);
        verify(control);
        tryVerify(function() { return control.implicitWidth > 0; });

        let published = null;
        control.menuRequested.connect(function(opener, anchor) {
            published = {"opener": opener, "anchor": anchor};
        });
        control.requestMenu();
        verify(published !== null);

        // The same corner, asked two ways: the item's own mapping, and the
        // scene arithmetic done by hand. Disagreement means the mapping
        // dropped the transform.
        const sceneOrigin = scaledScene.mapToGlobal(0, 0);
        const centreInScene = control.x + control.width / 2;
        const expected = sceneOrigin.x + centreInScene * 1.15;
        const publishedCentre = published.opener.x + published.opener.width / 2;
        verify(Math.abs(publishedCentre - expected) < 1.0,
               "published centre " + publishedCentre + ", expected " + expected);
        // And the published width is the real drawn width, not the layout one.
        verify(Math.abs(published.opener.width - control.width * 1.15) < 1.0,
               "published width " + published.opener.width
               + ", control width " + control.width);
        control.destroy();
    }

    function test_the_phone_publishes_a_real_opener() {
        const rects = testCase.openerOf(phoneComponent);
        verify(rects.opener.width > 0,
               "opener width " + rects.opener.width);
        verify(rects.opener.height > 0,
               "opener height " + rects.opener.height);
        verify(rects.anchor.width > 0);
        verify(rects.anchor.height > 0);
        verify(rects.anchor.width <= rects.opener.width + 0.001);
    }
}
