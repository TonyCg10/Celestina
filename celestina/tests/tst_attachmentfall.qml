import CelestinaStyle
import QtQuick
import QtTest
import "../qml" as Desktop

// PANEL-1-J. An attached surface is born hanging at its own seam, full size,
// and falls into place. What matters here is not the curve but the contract
// around it: the settled geometry is the destination, reduced motion never
// animates, and the carried content falls with its card.
TestCase {
    id: testCase

    name: "AttachmentFall"
    when: windowShown
    visible: true
    width: 700
    height: 500

    Desktop.BackdropInk {
        id: testInk
    }

    Component {
        id: fieldComponent

        Desktop.SoftMenuField {
            ink: testInk
            width: 420
            height: 300
            attachedToTop: true
            openerRect: Qt.rect(300, 5, 30, 30)
            attachmentAnchorRect: Qt.rect(306, 11, 18, 18)
            attachmentStartY: 40
            surfacePosition: Qt.point(120, 72)
        }
    }

    function fieldWith(reducedMotion) {
        return fieldComponent.createObject(
            testCase, {"reducedMotion": reducedMotion});
    }

    function bodyOf(field) {
        return findChild(field, "celestina-soft-menu-body-window");
    }

    function near(actual, expected) {
        verify(Math.abs(actual - expected) < 0.001,
               "expected " + expected + ", got " + actual);
    }

    function test_reduced_motion_opens_at_the_settled_geometry() {
        const field = testCase.fieldWith(true);
        verify(field);
        verify(field.edgeShapeActive);
        verify(!field.fallsIntoPlace);
        // No frame of an animation exists for this route: the surface is
        // already where it belongs before anything is revealed.
        compare(field.attachmentProgress, 1);
        compare(field.attachmentContentOpacity, 1);
        compare(testCase.bodyOf(field).opacity, 1);

        field.reveal();
        compare(field.attachmentProgress, 1);
        field.destroy();
    }

    function test_a_falling_surface_is_born_at_the_seam_and_lands_settled() {
        const field = testCase.fieldWith(false);
        verify(field);
        verify(field.edgeShapeActive);
        verify(field.fallsIntoPlace);

        // Born hanging at the bar: full size, carrying its content, above
        // its resting place by nearly the whole connector travel.
        compare(field.attachmentProgress, 0);
        const born = field.edgeSilhouette;
        verify(born.path.length > 0);
        near(born.openRect.width, field.width);
        near(born.openRect.height, field.height);
        const seamAtField = field.attachmentStartY - field.surfacePosition.y;
        verify(field.attachmentBodyRect.y < 0);
        verify(field.attachmentBodyRect.y >= seamAtField);

        field.reveal();
        // It lands on exactly the settled geometry and never past it.
        tryVerify(function() { return !field.attachmentClipsContent; });
        compare(field.attachmentProgress, 1);
        compare(field.attachmentBodyRect.y, 0);
        compare(field.attachmentContentOpacity, 1);
        const settled = field.edgeSilhouette;
        // The seam contact is the same at both ends of the fall.
        compare(settled.mouthLeft, born.mouthLeft);
        compare(settled.mouthRight, born.mouthRight);
        field.destroy();
    }

    function test_the_carried_content_rides_inside_the_drop() {
        const field = testCase.fieldWith(false);
        verify(field);
        const window = testCase.bodyOf(field);
        verify(window);
        // The content layer keeps its own settled layout inside that window,
        // so nothing it carries is stretched or reflowed by the motion.
        const content = findChild(field, "celestina-soft-menu-body");
        verify(content);
        compare(content.width, field.width);
        compare(content.height, field.height);

        // Born: the window is the full-sized body hanging at the seam, so
        // the content starts above its resting place and falls with the
        // glass rather than waiting for it.
        verify(field.attachmentClipsContent);
        verify(window.clip);
        compare(window.width, field.width);
        compare(window.height, field.height);
        verify(window.y < 0);

        field.reveal();
        // The recoil approaches 1 from above, so a fuzzy progress comparison
        // matches while the membrane is still relaxing. Wait for the surface
        // to declare itself settled instead.
        tryVerify(function() { return !field.attachmentClipsContent; });
        compare(field.attachmentProgress, 1);
        // Landed: the window is exactly the card again and stops clipping,
        // so a settled surface pays nothing for the motion.
        compare(window.x, 0);
        compare(window.y, 0);
        compare(window.width, field.width);
        compare(window.height, field.height);
        verify(!window.clip);
        field.destroy();
    }

    function test_a_settled_surface_never_falls_again() {
        const field = testCase.fieldWith(false);
        verify(field);
        field.reveal();
        tryCompare(field, "attachmentProgress", 1);
        // A route that reveals twice must not replay the opening under the
        // user, and a lease refresh must not either.
        field.reveal();
        compare(field.attachmentProgress, 1);
        field.beginDropFall();
        compare(field.attachmentProgress, 1);
        field.destroy();
    }

    function test_a_floating_surface_has_no_fall_at_all() {
        const field = fieldComponent.createObject(
            testCase, {"reducedMotion": false, "attachedToTop": false});
        verify(field);
        verify(!field.edgeShapeActive);
        verify(!field.fallsIntoPlace);
        compare(field.attachmentProgress, 1);
        compare(field.attachmentContentOpacity, 1);
        field.destroy();
    }
}
